---
name: Monitor Batch Plan Implementation
description: Watch a batch-plan-impl orchestrator session from this separate monitor session until ALL its plans are done — arm a background sleep timer (20 min) after every check (the harness wakes you when it fires; never poll it), judge liveness from real message-stream timestamps in the OpenCode SQLite DB (never the lagging session table — a quiet worker may be deep inside a bench/build tool call), nudge the orchestrator to continue ONLY on a real stop (failure, provider outage, ~45-min no-growth stall, or a pending question form, which you answer yourself via the form reply API picking the recommended option) using `opencode run --session <id>`; re-arm after every healthy check and finish with a final scoreboard once the orchestrator reports the plan queue empty. No user interaction: observe and report, intervene minimally, never implement anything yourself.
---

# Monitor Batch Plan Implementation

Be the **outer watchdog** for a `batch-plan-impl` orchestrator session. The
orchestrator dispatches worker sessions per phase, each with a watch-only
watcher subagent; the orchestrator itself goes idle and its watchers wake it
on completion. That machinery usually self-drives for hours. Your job is to
notice when it genuinely stops — crashed, stalled, blocked on a question —
and restart it with a nudge. You never implement, never touch git state,
never create or delete sessions.

## Stance

| You (monitor session) | Do | Never do |
| --- | --- | --- |
| | poll on a 20-min timer, judge liveness, report progress, nudge on real stops, answer pending question forms, post the final scoreboard | implement anything, edit files, commit/push, create/delete sessions, author task instructions, ask the user anything |

## Inputs

- **Target session id** (`$ORCH`) — normally given in the user message
  (`ses_...`). If not, auto-detect: the most recently active session in this
  project whose newest messages show batch-plan-impl activity (phase
  dispatches, a plan queue, run logs under `/tmp/opencode/ease-batch-plan-impl/run.md`).
  State your choice explicitly.
- **Interval** — 20 min (1200 s) unless the user says otherwise.
- **Baseline** — plan queue `~/.opencode/plan/*.md`, the orchestrator's run
  logs `/tmp/opencode/ease-batch-plan-impl/run.md`, current git HEAD. Completed plan files
  are never removed, so file existence is NOT the done-signal; "done" is the
  orchestrator's own report plus nothing left in flight.
- **Model refs** — `models.jsonc` at the project root defines the one pair
  `main.complex` / `main.balance` (full `providerID/modelID`). The batch pins
  workers and reviews to `main.complex`, watchers and THIS monitor session to
  `main.balance`. There are no switches to enforce anymore — if a fresh
  worker shows a different pin, note the drift in your report (the
  orchestrator owns pinning).

## Rules (each one was learned the hard way)

- **The timer is a background shell command**, not a loop:
  `sleep 1200 && echo "WAKE_UP_CHECK $(date +%H:%M:%S)"` with background on.
  The harness notifies you when it completes — that notification IS your
  wake-up. Never poll its output file, never use blocking waits, and always
  re-arm before ending a turn (except the final one).
- **Liveness = newest message time, not the session table.** Read the DB
  read-only (`~/.local/share/opencode/opencode.db`; the `sqlite3` CLI may be
  absent — use `python3` + the `sqlite3` module with URI `file:...?mode=ro`,
  which is WAL-safe). `session_v2.time_updated`/`time_idle` do NOT tick while
  a session streams inside a long tool call — a worker silent 30 min may be
  mid-build. Use `MAX(session_message.time_created)` per session.
- **A stall is real only when ALL hold**: the newest assistant message has a
  tool call stuck in a running state for ~45+ min, AND nothing is being
  written (no growth in the task's declared output files), AND the session is
  not idle. Id-unchanged alone is not death; parts must stop growing too.
- **Outage signature**: worker + watcher + orchestrator all dying within ~2
  minutes is a provider/system outage, not a work failure. Wait one cycle,
  then send a continue nudge — the orchestrator recovers by ADOPTING the dead
  worker's half-done work rather than discarding it.
- **A wake-up firing far later than scheduled means the machine was
  suspended.** Re-check state and carry on; do not assume failure.
- **The queue can grow mid-watch** — the user may feed the orchestrator new
  plans while you watch. Track the growth; keep watching until everything
  completes.
- Keep each wake-up report to the user SHORT: current phase/queue status in a
  few lines + "Next check at ~HH:MM".
- The watch may span days and several "Continue"-style user messages. On any
  resume: re-run the baseline check first, then keep the same rules.

## Decision policy (applied at every wake-up)

| Observation | Meaning | Action |
| --- | --- | --- |
| Orchestrator or its workers/watchers have fresh messages (< ~5–10 min) | working | 2–3 line progress note; re-arm timer |
| Orchestrator idle but a watcher of its current phase is still polling | by design — the watcher will wake it | re-arm timer, no nudge |
| Everything quiet < ~45 min, plans unfinished | suspicious | investigate before deciding: newest message types, running tool calls, output-file growth |
| Genuine stall (rule above) or `idle_outcome != succeeded` with plans unfinished | stopped | nudge to continue (below), then re-arm |
| Several sessions died within ~2 min of each other | provider outage | note it; next cycle nudge "continue"; expect an adopt-don't-clean recovery |
| A session has a pending question form and is otherwise quiet | blocked on a question | answer it yourself (below), verify resumption next wake |
| Orchestrator reports the queue empty / all plans done, nothing in flight | finished | final scoreboard; do NOT re-arm |

## Workflow

### 1. Baseline (first arming, and after every resume)

1. Record `$ORCH`, `$PROJECT_DIR` (the target's directory), interval.
2. Read the target's ~30 newest messages to learn the mission, the phase
   currently in flight, and the remaining plan queue.
3. Check the baseline: plan queue `~/.opencode/plan/`, run logs
   `/tmp/opencode/ease-batch-plan-impl/run.md`, `git -C $PROJECT_DIR log --oneline -5`.
4. Use the bundled status script for the recurring checks:

   ```sh
   bash "<skill-dir>/scripts/watch-status.sh" "$ORCH" "$PROJECT_DIR"
   ```

   It prints the target's state, its newest assistant text, every session
   active in the last 40 min with TRUE liveness, running-tool-call hints,
   pending question forms, and git state. Copy it to
   `/tmp/opencode/watch_<ses7>.sh` if you need to adapt the queries mid-watch.

### 2. Arm the first timer, then report the setup

```sh
sleep 1200 && echo "WAKE_UP_CHECK $(date +%H:%M:%S)"   # background
```

Then tell the user, in a few lines: the watched session and title, its
remaining queue, the nudge rule ("I nudge only if everything goes quiet with
plans unfinished"), and the next check time. End the turn.

### 3. Every wake-up

1. Run the status script.
2. Classify per the decision policy; investigate before any nudge.
3. Re-arm the timer (unless done).
4. One short report: what is in flight / landed since last check (commit
   hashes when the run log shows them), any switch applied or verified,
   + "Next check at ~HH:MM".

### 4. Nudge protocol (the only intervention channel that always works)

```sh
cd "$PROJECT_DIR" && opencode run --session "$ORCH" "<nudge text>"
```

- `subagent` with `sessionID` works only for YOUR OWN child sessions — the
  orchestrator is not one, so that route fails; do not rely on it.
- `opencode api` handles server auth itself; no API key plumbing is needed.
- Nudge text template — identify yourself, state facts, defer to its run log:

  ```text
  Continue (monitor nudge from watch session <YOUR_SESSION_ID>, tasked with
  watching you until all plans are done). <Factual observation: what failed
  or stalled, when, which session ids, what the run log says — e.g. "your
  <plan> phase <n> worker <ses_...> and watcher <ses_...> died at HH:MM
  within 2 minutes of each other; HEAD is still at phase <n-1>".> Resume per
  your own run log and plan queue. No new instructions.
  ```

- Never add new task instructions in a nudge — restart the orchestrator's own
  plan, nothing else.

### 5. Question-form unblock (a worker "running" ~1 h with zero output)

A tool call stuck "running" for ~1 h with no files written is often a pending
question, not compute. Check and answer it yourself:

```sh
# 1. List pending forms (empty list = none):
opencode api get "/api/session/$WORKER/form"

# 2. Inspect fields and options:
opencode api get "/api/session/$WORKER/form" \
  | python3 -c "import json,sys; d=json.load(sys.stdin)['data'][0]; \
print(d['id']); [print(f['key'],'|',f['title'],'|',[o['value'] for o in f.get('options',[])]) for f in d['fields']]"

# 3. Reply — body key is "answer"; pick the "(Recommended)" option, or the
#    option the orchestrator's delegation message named:
opencode api post "/api/session/$WORKER/form/$FORM_ID/reply" \
  -d '{"answer":{"q0":"examples/README.md (Recommended)"}}'
```

Then verify the pending-forms list is empty, and check on the next wake that
the worker actually resumed (if it did not, nudge the orchestrator about it —
an injected answer can queue behind a still-pending question).

### 6. Finish — all plans done

Confirm ALL of: the orchestrator's newest assistant text reports the queue
empty / all plans handled (its final or morning report), no worker/watcher
sessions remain active, and the tree is clean and pushed. Then post the final
summary and arm NO new timer:

```text
# ✅ Watch complete — all plans are done
<the orchestrator's closing claim, quoted>

| Batch | Verdict | Headline |
| ...   | ...     | ... |

Total span: <base> → <HEAD> — N commits; watch duration ~Xh, N checks at
20-minute intervals; nudges sent: <n> (<why>).
Nothing is pending — no timer is armed.
```

## Quick command reference

| Action | Command |
| --- | --- |
| Wake-up status check | `bash "<skill-dir>/scripts/watch-status.sh" "$ORCH" "$PROJECT_DIR"` |
| Arm the timer (background) | `sleep 1200 && echo "WAKE_UP_CHECK $(date +%H:%M:%S)"` |
| Model refs | `sed 's://.*$::' models.jsonc \| jq -r '.main.complex, .main.balance'` — workers/reviews on complex, watchers + this monitor on balance |
| Worker pins (drift note only) | `session_v2.model` of fresh workers (watch-status.sh prints it) — expect `main.complex` workers, `main.balance` watchers; drift goes in the report, not a nudge |
| True liveness per session | `MAX(session_message.time_created)` via read-only sqlite (python3; `sqlite3` CLI may be missing) |
| Newest message of a session | `SELECT type, data FROM session_message WHERE session_id=? ORDER BY seq DESC LIMIT 1` |
| Nudge the orchestrator | `cd "$PROJECT_DIR" && opencode run --session "$ORCH" "<facts + continue>"` |
| Pending question forms | `opencode api get "/api/session/$SID/form"` |
| Answer a form | `opencode api post "/api/session/$SID/form/$FORM/reply" -d '{"answer":{"q0":"<option>"}}'` |
| Interrupt (only if the orchestrator's own policy requires it — it owns retries) | leave to the orchestrator; you never interrupt |

## Notes

- Read-only DB access is deliberate: `file:...?mode=ro` cannot corrupt the
  live WAL. Never write to the DB directly; all message injection goes
  through `opencode run --session` or the form reply API.
- `opencode api` flags: `--data/-d <body>`, `--header name:value`,
  `--param key=value`. `opencode run` flags: `--session/-s <id>`,
  `--format json`.
- Run log locations to consult for progress: `/tmp/opencode/ease-batch-plan-impl/run.md`
  (per-batch phase log) and `~/.opencode/plan/*.md` (the plan queue — the
  orchestrator adds files there as the user requests more work).
- You are NOT the orchestrator: never load or imitate the `batch-plan-impl`
  skill's worker/phase behaviors. If the target session cannot be found or
  its plans cannot be identified, report that and stop — do not guess.
