// tur:rpc — the runtime half of `ease-tur-rpc` (the Rust caller lives in
// `lib.rs`). Registered by `TurRpcPlugin::register` as the synthetic module
// "tur:rpc"; plugins import { hostRpc, viewRpc } from "tur:rpc".
//
// Bus channel ids (RPC_CH/EVENT_CH/CREDIT_CH) and the stream-frame magic
// bytes must stay in sync with the Rust constants in lib.rs.
import { eventBus, encodeUtf8, decodeUtf8 } from "tur:std";

// Bus channel ids — must match the Rust constants RPC_CHANNEL_ID (0),
// EVENT_CHANNEL_ID (1) and CREDIT_CHANNEL_ID (2) in this crate.
const RPC_CH = 0;
const EVENT_CH = 1;
const CREDIT_CH = 2;

const hostHandlers = new Map();       // op -> hostRpc.registerHandler fn
const viewHandlers = new Map();       // op -> viewRpc.registerHandler fn
const streamHandlers = new Map();     // op -> hostRpc.registerStream opener fn
const eventHandlers = new Map();      // type -> hostRpc.onEvent fn

// --- credit gates (flow control + cancellation, host -> JS on CREDIT_CH) --
// One per open stream: the counting semaphore a StreamProducer.push acquires.
// Credits arrive as {"sid",n}; cancels as {"sid",cancel:true}.
const gates = new Map(); // sid -> { credits, waiters, cancelled }

function gateFor(sid) {
  let g = gates.get(sid);
  if (!g) {
    g = { credits: 0, waiters: [], cancelled: false };
    gates.set(sid, g);
  }
  return g;
}

function takeCredit(sid) {
  return new Promise((resolve) => {
    const g = gates.get(sid);
    if (!g || g.cancelled) return resolve(false); // aborted -> stop pumping
    if (g.credits > 0) {
      g.credits -= 1;
      return resolve(true);                       // fast path, no parking
    }
    g.waiters.push(resolve);                      // park until grant/cancel
  });
}

eventBus.on(CREDIT_CH, (payload) => {
  let m;
  try {
    m = JSON.parse(decodeUtf8(payload));
  } catch (e) {
    return;
  }
  if (!m || typeof m.sid !== "number") return;
  if (m.cancel) {
    const g = gateFor(m.sid);
    g.cancelled = true;
    const ws = g.waiters.splice(0);
    for (const w of ws) w(false);
    return;
  }
  if (typeof m.n !== "number" || m.n <= 0) return;
  const g = gateFor(m.sid);
  g.credits += Math.floor(m.n);
  while (g.waiters.length > 0 && g.credits > 0) {
    g.credits -= 1;
    g.waiters.shift()(true);
  }
});

// --- control RPC (JSON on RPC_CH) -----------------------------------------

function reply(id, ok, rest) {
  // `rest` is the result on success, the error message on failure — named
  // per key below so a failed call with an undefined message still replies.
  const msg = ok ? { id: id, ok: true, result: rest }
                 : { id: id, ok: false, error: rest };
  eventBus.send(RPC_CH, encodeUtf8(JSON.stringify(msg)));
}

function invoke(id, fn, args) {
  Promise.resolve()
    .then(() => fn(args))
    .then(
      (result) => reply(id, true, result),
      (err) => reply(id, false, String((err && err.message) || err)),
    );
}

eventBus.on(RPC_CH, (payload) => {
  let req;
  try {
    req = JSON.parse(decodeUtf8(payload));
  } catch (e) {
    return; // not a JSON control message (stream frames are binary)
  }
  if (!req || typeof req.id === "undefined" || typeof req.op !== "string") return;

  // Envelope-level stream id: present only for stream opens (host-only —
  // views cannot open streams). `args` is the host's verbatim payload, never
  // a magic-key carrier.
  const sid = typeof req.streamId === "number" ? req.streamId : null;

  // Scope: which registration namespace serves the op. The Rust client
  // always sends one; anything else is a malformed envelope.
  if (req.scope === "host") {
    if (sid !== null) {
      // Stream opens are served exclusively by hostRpc.registerStream.
      const open = streamHandlers.get(req.op);
      if (typeof open === "function") {
        dispatchStream(req, sid, open);
      } else {
        reply(req.id, false, "no stream handler for op: " + req.op);
      }
      return;
    }
    const fn = hostHandlers.get(req.op);
    if (typeof fn !== "function") {
      reply(req.id, false, "no host handler for op: " + req.op);
      return;
    }
    invoke(req.id, fn, req.args);
    return;
  }

  if (req.scope === "view") {
    if (sid !== null) {
      reply(req.id, false, "malformed rpc envelope: view scope cannot open streams");
      return;
    }
    const fn = viewHandlers.get(req.op);
    if (typeof fn !== "function") {
      reply(req.id, false, "no view handler for op: " + req.op);
      return;
    }
    invoke(req.id, fn, req.args);
    return;
  }

  reply(req.id, false, 'malformed rpc envelope: scope must be "host" or "view"');
});

// --- registration namespaces ----------------------------------------------
// hostRpc: everything the HOST may invoke on this backend — the storage
// contract (`storage:list` / `storage:get`), the instance lifecycle
// (`storage:removeInstance`), the OAuth flow (`oauth:url` /
// `oauth:exchange`), and host-fired events (`music:play`). Ops are contract
// literals, identical for every provider; identity rides the payload.
// viewRpc: ops this plugin's own VIEW reaches via `ease.rpc.call`. Views
// are JSON request/response only, so there is no registerStream/onEvent
// here. An op callable from both sides is simply registered in both.
export const hostRpc = {
  registerHandler(op, fn) {
    hostHandlers.set(op, fn);
  },
  registerStream(op, open) {
    streamHandlers.set(op, open);
  },
  onEvent(type, fn) {
    eventHandlers.set(type, fn);
  },
};

export const viewRpc = {
  registerHandler(op, fn) {
    viewHandlers.set(op, fn);
  },
};

// --- plugin events (fire-and-forget, host -> JS) --------------------------
// Handlers come from hostRpc.onEvent above; frames arrive as
// {type, payload} JSON on the dedicated event channel.
eventBus.on(EVENT_CH, (payload) => {
  let ev;
  try {
    ev = JSON.parse(decodeUtf8(payload));
  } catch (e) {
    return;
  }
  if (!ev || typeof ev.type !== "string") return;
  const fn = eventHandlers.get(ev.type);
  if (typeof fn === "function") fn(ev.payload);
});

// --- streaming ------------------------------------------------------------
// Binary framing: [magic u8][streamId u32 LE][...payload]. Magic values
// must match the Rust constants (0=chunk, 1=end, 2=error).
function frameStream(kind, streamId, payload) {
  const bodyLen = payload ? payload.length : 0;
  const out = new Uint8Array(5 + bodyLen);
  out[0] = kind;
  const dv = new DataView(out.buffer);
  dv.setUint32(1, streamId >>> 0, true); // little-endian
  if (payload) out.set(payload, 5);
  return out;
}

function sendChunk(sid, bytes) {
  eventBus.send(RPC_CH, frameStream(0, sid, bytes));
}
function sendEnd(sid) {
  gates.delete(sid);
  eventBus.send(RPC_CH, frameStream(1, sid, null));
}
function sendError(sid, message) {
  gates.delete(sid);
  eventBus.send(RPC_CH, frameStream(2, sid, encodeUtf8(String(message))));
}

// StreamProducer — the sid-bound object the pump drives. The sid is captured
// here and nowhere else; nothing above this layer touches stream ids.
function producerFor(sid) {
  const closed = { v: false };
  return {
    async push(bytes) {
      if (closed.v) return false;
      if (!(await takeCredit(sid))) {
        closed.v = true;
        return false; // host cancelled the stream — stop pumping
      }
      sendChunk(sid, bytes);
      return true;
    },
    end() {
      if (!closed.v) {
        closed.v = true;
        sendEnd(sid);
      }
    },
    error(message) {
      if (!closed.v) {
        closed.v = true;
        sendError(sid, message);
      }
    },
  };
}

function asyncIterableOf(body) {
  if (body && typeof body[Symbol.asyncIterator] === "function") return body;
  return { [Symbol.asyncIterator]() { return body; } };
}

// The pump: pull the opener's body, credit-gate every push, and own ALL exit
// paths (normal end, host cancel, mid-body error) via try/catch/finally.
async function pump(sid, body, hooks) {
  const out = producerFor(sid);
  try {
    for await (const chunk of asyncIterableOf(body)) {
      if (!(await out.push(chunk))) break; // cancelled
    }
    out.end(); // normal AND cancel exit; gate cleanup lives in end()/error()
  } catch (e) {
    const err = hooks.mapError ? hooks.mapError(e) : e;
    out.error(String((err && err.message) || err));
  } finally {
    // body.return?.() is a courtesy close (native tur:net bodies don't
    // implement it; the release hook below is the real abort).
    if (body && typeof body.return === "function") {
      const r = body.return();
      if (r && typeof r.catch === "function") r.catch(() => {});
    }
    if (hooks && typeof hooks.release === "function") {
      try { hooks.release(); } catch (e) { /* release must not throw */ }
    }
  }
}

function dispatchStream(req, sid, open) {
  // The gate must exist before the initial grant can arrive (the host emits
  // it before the request; bus delivery is FIFO within a flush).
  gateFor(sid);
  Promise.resolve()
    .then(() => open(req.args)) // args verbatim — no streamId anywhere
    .then((source) => {
      const body = source && source.body;
      const iterable = !!body && (typeof body.next === "function" ||
        typeof body[Symbol.asyncIterator] === "function");
      if (!iterable) {
        // Thrown here, caught by the final handler below → ok:false reply.
        throw new Error("registerStream opener must resolve { meta, body }");
      }
      return source;
    })
    .then(
      (source) => {
        const meta = source.meta !== undefined ? source.meta : {};
        // Metadata first — the host's open_stream resolves on this reply,
        // then drains the chunks as they (and their credits) arrive.
        reply(req.id, true, meta);
        pump(sid, source.body, {
          release: source.release,
          mapError: source.mapError,
        });
      },
      (err) => {
        // Single error path: fail the RPC itself. Nothing was streamed, so
        // no terminal frame is needed.
        gates.delete(sid);
        reply(req.id, false, String((err && err.message) || err));
      },
    );
}

