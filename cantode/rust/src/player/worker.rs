//! The worker: event loop and the operations it dispatches to.
//!
//! The worker (not the public API) is the sole owner of the decoder and
//! the sink. This keeps the cpal `Stream` on a single thread for its
//! whole lifetime — the discipline cpal/AAudio/CoreAudio require for
//! real-time audio.
//!
//! State transitions are *requested* from the
//! [`Machine`](super::phase::Machine) — the worker names intents
//! (`play`, `pause`, `fail`, …), never causes or phases. The worker's
//! own writes are the session-independent observables (`position`,
//! `duration`) and the operational events (`MetadataReady`,
//! `PositionChanged`, `Ended`).
//!
//! The loop parks in `recv_timeout`: **5 ms while `Playing` or
//! `Buffering`** (so decode work / refill polls interleave with command
//! handling — a command arriving mid-wait wakes the loop instantly; the
//! timeout is the idle fallback, not added latency), **250 ms while
//! `Paused` with a buffering source** (keeping the buffered-window
//! observable live while the source's session thread fills the readahead
//! window in the background), and one hour otherwise (pure event-driven
//! idling, with the timeout acting only as a watchdog). Each timeout tick
//! pumps exactly one frame while playing, polls the source's readiness
//! while buffering, and refreshes the buffered-window mirror while
//! paused. The pump loop is a
//! buffer-*filler*, not the pacer: the sink's blocking write matches
//! decode speed to playback speed once its ring is full, and the 5 ms
//! quantum just guarantees the worker re-enters `recv_timeout` between
//! every frame so queued commands preempt within one pump. A starved
//! starved source surfaces as `Playing → Buffering` (readiness pre-check, or the
//! 250 ms play-path read deadline) and back on refill — the sink keeps
//! draining its ring across the morph. When decode hits EOF on a
//! position-tracking sink, the loop keeps ticking as a **tail drain**:
//! `Ended` fires only once the listener has actually heard the decoded
//! tail — the output position reaches the end of what was decoded, or
//! the sink's ring is found empty under a frozen clock (bookkeeping
//! drift between container timestamps and counted samples), or the
//! no-progress stall budget lapses.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AudioSource, CantodeError, Metadata, decoder::DecoderFactory, events::PlayerEvent,
    output::AudioSinkFactory,
};

use super::EventSinks;
use super::buffered_media_time;
use super::command::{Command, LoadResult, SeekResult};
use super::phase::Machine;
use super::session::{Loaded, PumpOutcome};
use super::shared::SharedStatus;

/// How often the worker emits [`PlayerEvent::PositionChanged`] while
/// playing. 10 Hz matches typical UI polling cadences and keeps the
/// event channel from saturating. Passed into `Loaded::pump` — event
/// cadence is the worker's policy, not the session's.
const POSITION_EMIT_INTERVAL: Duration = Duration::from_millis(100);

/// How long the end-of-stream tail drain may make no output progress
/// (device stalled mid-drain with audio still in the ring) before
/// giving up and ending anyway. Only the wedged-device path waits this
/// out: a frozen clock whose ring has emptied (or that sits within
/// [`DRAIN_SETTLE_TOLERANCE`] of the target) is the *normal* end and
/// finishes immediately — see [`Worker::drain_tick`].
const DRAIN_MAX_STALL: Duration = Duration::from_secs(8);

/// A gap between the ts-derived drain target and the frozen output
/// clock within this window is bookkeeping drift (container timestamps
/// vs counted samples), not unheard audio: settle instead of waiting
/// out the stall budget. Larger real shortfalls end via the sink's
/// empty-ring report (or the stall budget when the sink can't report).
const DRAIN_SETTLE_TOLERANCE: Duration = Duration::from_millis(200);

/// How often the worker refreshes the buffered-window observable while
/// `Paused` with a buffering source. The 5 ms playing/buffering ticks
/// refresh it on every pass anyway; this slow tick exists so a paused
/// player still reports a window that keeps filling in the background
/// (the source's session thread is independent of transport state).
const BUFFERED_REFRESH_INTERVAL: Duration = Duration::from_millis(250);

/// Tail-drain state: decode reached EOF, but the sink still holds up to
/// its full ring buffer (~3 s) of decoded-but-unheard audio.
/// [`PlayerEvent::Ended`] must wait until the listener has actually heard
/// the end, so the worker keeps its short tick and watches the sink's
/// realtime output position.
struct Drain {
    /// Media time just past the last decoded frame — when the output
    /// position reaches this, the tail has sounded.
    target: Duration,
    /// Last observed output position (progress detection).
    last_pos: Duration,
    /// When output progress was last observed; `None` while advancing.
    /// Armed on the first frozen tick that neither the settle tolerance
    /// nor the empty-ring check already resolves; the drain gives up
    /// once `DRAIN_MAX_STALL` elapses without progress (wedged device,
    /// or a sink that can't report occupancy).
    stalled_since: Option<Instant>,
    /// What the post-drain landing is: the normal EOF drain ends the
    /// track; the hard-source-error drain parks it (the error message
    /// is already poll-visible by then).
    then: DrainEnd,
}

enum DrainEnd {
    End,
    Pause,
}

pub(super) struct Worker {
    /// The state-machine core: owns the phase and the shared state
    /// mirror. The worker only requests transitions from it.
    machine: Machine,
    decoder_factory: Arc<dyn DecoderFactory>,
    cmd_rx: mpsc::Receiver<Command>,
    /// Constructs the sink for each loaded source (custom, or the default
    /// cpal device sink when the config didn't provide one).
    sink_factory: AudioSinkFactory,
    /// Observable projection shared with the `Player` handle. The worker
    /// only publishes `duration` here (after a successful load);
    /// `state` belongs to the machine, `position` to the session.
    shared: Arc<SharedStatus>,
    /// Operational events (`MetadataReady`, `PositionChanged`, `Ended`).
    /// `StateChanged` / illegal-transition errors come from the machine's
    /// own copy.
    sinks: EventSinks,
    /// Dedup latch for source-error events: a failing source errors on
    /// every pump; the UI wants one `PlayerEvent::Error` per episode,
    /// not a stream of them. Cleared by a successful seek (the classic
    /// user-driven recovery).
    error_latched: bool,
    /// Active end-of-stream tail drain (see [`Drain`]). Lives only while
    /// the phase stays `Playing`; cleared by any phase-changing command
    /// (pause/seek/stop/load) — a pause, for instance, freezes the
    /// output clock, and the re-pump after resume re-arms the drain.
    drain: Option<Drain>,
    /// The configured [`PlayerConfig::min_buffer_duration`]: playback
    /// start and underrun resume wait for this much buffered-ahead
    /// media time (see [`Worker::buffered_ahead_sufficient`]).
    min_buffer: Duration,
}

impl Worker {
    /// Assemble a worker. Called by `Player::with_config` inside the
    /// spawn closure; the fields stay private to this module.
    pub(super) fn new(
        machine: Machine,
        decoder_factory: Arc<dyn DecoderFactory>,
        cmd_rx: mpsc::Receiver<Command>,
        sink_factory: AudioSinkFactory,
        shared: Arc<SharedStatus>,
        sinks: EventSinks,
        min_buffer: Duration,
    ) -> Self {
        Self {
            machine,
            decoder_factory,
            cmd_rx,
            sink_factory,
            shared,
            sinks,
            error_latched: false,
            drain: None,
            min_buffer,
        }
    }

    pub(super) fn run(&mut self) {
        loop {
            let timeout = self.next_timeout();
            match self.cmd_rx.recv_timeout(timeout) {
                Ok(cmd) => {
                    if self.handle_command(cmd) {
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if self.machine.is_playing() {
                        self.pump_once();
                    } else if self.machine.is_buffering() {
                        self.poll_refill();
                    } else if self.machine.is_paused() {
                        // No pipeline work while paused — just keep the
                        // buffered-window mirror live while the source's
                        // session thread fills the readahead window.
                        self.refresh_buffered();
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // All senders dropped (Player::Drop). Bail.
                    return;
                }
            }
        }
    }

    /// The park duration for the next `recv_timeout` iteration.
    ///
    /// - 5 ms while `Playing` or `Buffering`, so decode work / refill
    ///   polls interleave with command handling — a command arriving
    ///   mid-wait wakes the loop instantly; the timeout is the idle
    ///   fallback, not added latency.
    /// - [`BUFFERED_REFRESH_INTERVAL`] while `Paused` **with a buffering
    ///   source**: its session thread keeps filling the readahead window
    ///   in the background, and the observable mirror must follow.
    /// - One hour otherwise (pure event-driven idling, with the timeout
    ///   acting only as a watchdog).
    fn next_timeout(&mut self) -> Duration {
        if self.machine.is_playing() || self.machine.is_buffering() {
            // Short timeout so we can interleave decode work.
            Duration::from_millis(5)
        } else if self.machine.is_paused() && self.session_reports_buffered() {
            BUFFERED_REFRESH_INTERVAL
        } else {
            Duration::from_secs(60 * 60)
        }
    }

    /// Whether the live session's source maintains a buffered window
    /// (`AudioSource::buffered_range`). Non-buffering sources (memory,
    /// local files) keep the worker's pure event-driven idle.
    fn session_reports_buffered(&mut self) -> bool {
        self.machine
            .loaded_mut()
            .is_some_and(|loaded| loaded.buffered_range().is_some())
    }

    /// Mirror the session source's buffered window into the shared
    /// observables. Called from the playing/buffering ticks and the slow
    /// paused tick — the read is a cheap lock on the source's state.
    fn refresh_buffered(&mut self) {
        if let Some(loaded) = self.machine.loaded_mut() {
            self.shared.set_buffered_range(loaded.buffered_range());
        }
    }

    /// Returns `true` if the worker should exit.
    fn handle_command(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::Shutdown => return true,
            Command::Load { source, reply, autoplay } => {
                self.drain = None;
                let result = self.do_load(source, autoplay);
                let _ = reply.send(match &result {
                    Ok(m) => LoadResult::Ok(m.clone()),
                    Err(e) => LoadResult::Err(e.clone()),
                });
            }
            Command::Play => self.machine.play(),
            Command::Pause => {
                self.drain = None;
                self.machine.pause();
            }
            Command::Stop => {
                self.drain = None;
                self.do_stop();
            }
            Command::Unload { reply } => {
                self.drain = None;
                let r = self.do_unload();
                let _ = reply.send(r);
            }
            Command::Seek { target, reply } => {
                self.drain = None;
                let r = self.do_seek(target);
                let _ = reply.send(match r {
                    Ok(d) => SeekResult::Ok(d),
                    Err(e) => SeekResult::Err(e),
                });
            }
            Command::SetVolume(v) => {
                if let Some(loaded) = self.machine.loaded_mut() {
                    loaded.set_volume(v);
                }
            }
        }
        false
    }

    fn do_load(&mut self, source: Box<dyn AudioSource>, autoplay: bool) -> crate::Result<Metadata> {
        // Discard any existing session (old sink stops, session-scoped
        // observables reset) and publish `Loading` — in that order, per
        // `Machine::begin_load`.
        self.machine.begin_load();

        let opened = self.decoder_factory.open(source);
        let dec = match opened {
            Ok(dec) => dec,
            Err(e) => {
                self.machine.fail();
                return Err(e);
            }
        };
        let meta = dec.metadata().clone();
        let fmt = dec.format();

        // Construct the sink (custom factory, or the default cpal device)
        // and open it. Both failure paths mirror each other: transition to
        // `Error` and propagate.
        let mut sink: Box<dyn crate::output::AudioSink> = match (self.sink_factory)() {
            Ok(sink) => sink,
            Err(e) => {
                self.machine.fail();
                return Err(e);
            }
        };
        let actual_fmt = match sink.start(fmt) {
            Ok(actual_fmt) => actual_fmt,
            Err(e) => {
                self.machine.fail();
                return Err(e);
            }
        };

        let loaded = Loaded::new(dec, sink, actual_fmt, Arc::clone(&self.shared));

        // Publish metadata/duration only once the sink is up (the success
        // path): a failed load leaves the observables cleared by the old
        // session's drop, rather than half-updated.
        self.shared.set_duration(meta.duration);
        self.sinks.emit(PlayerEvent::MetadataReady(meta.clone()));

        // Commit the fresh session as `Paused` — or straight into
        // `Playing` for an autoplay load (validated against the
        // transition table inside the machine).
        self.machine.complete_load(loaded, autoplay);
        // Startup prebuffer: an autoplay load whose cushion is thinner
        // than `min_buffer_duration` parks in `Buffering` (the same
        // legal `Playing → Buffering` morph an underrun takes) until
        // the refill poll sees the threshold. Starting on a thin window
        // is exactly the "plays a beat, buffers, plays a beat" stutter
        // this exists to prevent. Non-window sources, unknown
        // durations, and windows that already cover the rest of the
        // track fall straight through to `Playing`.
        if autoplay && !self.buffered_ahead_sufficient() {
            tracing::info!(
                min_buffer_ms = self.min_buffer.as_millis() as u64,
                "startup prebuffer armed"
            );
            self.machine.buffer_underrun();
        }
        Ok(meta)
    }

    fn do_stop(&mut self) {
        // `StopRequested → Idle`. The machine drops the session (if any),
        // which stops the sink and resets position/duration. From `Idle`
        // the transition is a no-op and nothing runs.
        self.machine.stop();
    }

    fn do_unload(&mut self) -> crate::Result<()> {
        self.do_stop();
        Ok(())
    }

    fn do_seek(&mut self, target: Duration) -> crate::Result<Duration> {
        let Some(loaded) = self.machine.loaded_mut() else {
            return Err(CantodeError::InvalidState(
                "seek requires a loaded source".into(),
            ));
        };
        // The session performs the choreography (decoder seek + sink
        // flush + latch clear + position publish); we announce it.
        let actual = loaded.seek(target)?;
        self.sinks.emit(PlayerEvent::PositionChanged(actual));
        // A successful seek is the classic user-driven recovery — the
        // source-error episode (if any) is over: the latch drops and the
        // poll-visible message clears (the seek itself already re-opened
        // the source with a fresh retry budget).
        self.error_latched = false;
        self.shared.set_source_error(None);
        Ok(actual)
    }

    /// Decode one frame and push it to the sink. Called from the
    /// Playing-loop body — also while a tail drain is pending, in which
    /// case the tick drives [`Worker::drain_tick`] instead of a decode.
    fn pump_once(&mut self) {
        // Every playing tick refreshes the buffered-window mirror.
        self.refresh_buffered();
        if self.drain.is_some() {
            self.drain_tick();
            return;
        }
        // Stage 1 — the session's decode→render step under the playing
        // borrow. It stores the position observable itself and returns
        // the emission decisions. The readiness pre-check skips the read
        // entirely when the window is starved (a read would park up to
        // the deadline and defer commands behind it).
        let outcome = {
            let Some((loaded, last_position_emit)) = self.machine.playing_mut() else {
                return;
            };
            if loaded.readiness() == crate::Readiness::NeedsData {
                PumpOutcome::NeedsData
            } else {
                loaded.pump(last_position_emit, POSITION_EMIT_INTERVAL)
            }
        };

        // Stage 2 — events and transitions, after the borrow ended.
        match outcome {
            PumpOutcome::Frame { position, emit } => {
                if emit {
                    self.sinks.emit(PlayerEvent::PositionChanged(position));
                }
            }
            PumpOutcome::EndOfStream => {
                // With a position-tracking sink, defer `Ended` until the
                // buffered tail has actually sounded (the ring holds up
                // to ~3 s of decoded-but-unheard audio — ending now would
                // cut every track's last seconds short and auto-advance
                // early). Sinks without tracking keep the historical
                // immediate end.
                let drain_target = self
                    .machine
                    .loaded_mut()
                    .and_then(|loaded| loaded.output_position().map(|_| loaded.decoded_through()));
                match drain_target {
                    Some(target) => {
                        // Early-decode-EOF trace: if the frontier is far
                        // short of the known duration, the decoder
                        // consumed the stream too fast (or truncated).
                        tracing::info!(
                            frontier_ms = target.as_millis() as u64,
                            duration_ms = self.shared.duration().map(|d| d.as_millis() as u64),
                            "tail drain armed"
                        );
                        self.drain = Some(Drain {
                            target,
                            last_pos: Duration::ZERO,
                            stalled_since: None,
                            then: DrainEnd::End,
                        });
                    }
                    None => self.finish_end_of_stream(),
                }
            }
            PumpOutcome::NeedsData => {
                // Starved but alive: park the pump (the sink drains its
                // ring) until the refill poll sees data again.
                self.machine.buffer_underrun();
            }
            PumpOutcome::Skipped(err) => {
                if let Some(e) = err {
                    // A hard source error (`CantodeError::Source`) is
                    // terminal — transient starvation surfaces as
                    // `WouldBlock` long before a hard error escapes the
                    // source. Publish the message for the poll (the
                    // embedder can react without an event subscription),
                    // then land on `Paused`: while `Playing` with a
                    // position-tracking sink the decoded tail first
                    // drains (same choreography as the EOF drain, minus
                    // the end — cutting already-buffered audio short
                    // would click), everywhere else the park is
                    // immediate (nothing is sounding). The position and
                    // session survive either way, and the next play is
                    // the user-driven retry (the app seeks first, which
                    // re-opens the source with a fresh retry budget).
                    // Decode errors keep the historical
                    // skip-and-continue.
                    if let CantodeError::Source(msg) = &e {
                        self.shared.set_source_error(Some(msg.clone()));
                        let drain_target = self.machine.loaded_mut().and_then(|loaded| {
                            loaded.output_position().map(|_| loaded.decoded_through())
                        });
                        match drain_target {
                            Some(target) => {
                                tracing::info!(
                                    frontier_ms = target.as_millis() as u64,
                                    "source-error drain armed"
                                );
                                self.drain = Some(Drain {
                                    target,
                                    last_pos: Duration::ZERO,
                                    stalled_since: None,
                                    then: DrainEnd::Pause,
                                });
                            }
                            None => self.machine.pause(),
                        }
                    }
                    self.report_source_error(e);
                }
            }
        }
    }

    /// One tail-drain tick: watch the sink's realtime output position
    /// until the tail has sounded, then end. "Has sounded" is any of:
    /// the position reached the decode frontier's end (the normal
    /// path); the position froze **and** the sink reports an empty ring
    /// — everything decoded has sounded, so any remaining gap to the
    /// target is bookkeeping drift between container timestamps and
    /// counted samples (the frozen-short-of-target end that used to
    /// wait out the full stall budget and added ~8 s of dead air after
    /// the last audible sample); the frozen position sits within
    /// [`DRAIN_SETTLE_TOLERANCE`] of the target (sub-perceptible
    /// drift); or no progress for [`DRAIN_MAX_STALL`] with audio still
    /// held (device wedged, or a sink that can't report occupancy).
    /// Gives up after [`DRAIN_MAX_STALL`] without progress.
    /// The observed position is mirrored into the observable so the
    /// progress glides to the end instead of freezing ~ring-fill short.
    fn drain_tick(&mut self) {
        // Disjoint-field borrows: the drain bookkeeping and the machine.
        let mut live_pos = None;
        let finish = match (self.drain.as_mut(), self.machine.loaded_mut()) {
            (Some(drain), Some(loaded)) => match loaded.output_position() {
                // The sink stopped reporting positions mid-drain — end now.
                None => true,
                Some(pos) if pos >= drain.target => {
                    live_pos = Some(pos);
                    true
                }
                Some(pos) => {
                    live_pos = Some(pos);
                    if pos > drain.last_pos {
                        drain.last_pos = pos;
                        drain.stalled_since = None;
                        false
                    } else {
                        // Output froze: normal end vs wedged device.
                        let settled =
                            drain.target.saturating_sub(pos) <= DRAIN_SETTLE_TOLERANCE;
                        let ring_empty = loaded.undrained().is_some_and(|d| d.is_zero());
                        if settled || ring_empty {
                            true
                        } else if drain.stalled_since.is_none() {
                            drain.stalled_since = Some(Instant::now());
                            false
                        } else {
                            drain.stalled_since.unwrap().elapsed() >= DRAIN_MAX_STALL
                        }
                    }
                }
            },
            // No session to drain (stop/load raced in) — end now.
            _ => true,
        };
        // Same live-position mirror as the Buffering tick: the pump is
        // parked, but the device keeps draining its ring.
        if let Some(pos) = live_pos {
            self.shared.set_position(pos);
        }
        if finish {
            match (&self.drain, live_pos) {
                (Some(drain), Some(pos)) => tracing::info!(
                    target_ms = drain.target.as_millis() as u64,
                    achieved_ms = pos.as_millis() as u64,
                    shortfall_ms = drain.target.saturating_sub(pos).as_millis() as u64,
                    "tail drain finished"
                ),
                (Some(drain), None) => tracing::info!(
                    target_ms = drain.target.as_millis() as u64,
                    "tail drain finished (sink stopped reporting positions)"
                ),
                _ => {}
            }
            match self.drain.take().map(|d| d.then) {
                Some(DrainEnd::Pause) => self.finish_drain_pause(),
                _ => self.finish_end_of_stream(),
            }
        }
    }

    /// Leave the playing phases into `Paused` after a drained hard
    /// source error: the tail has sounded, the error message is already
    /// poll-visible, and the next play is the user-driven retry.
    fn finish_drain_pause(&mut self) {
        self.drain = None;
        self.machine.pause();
    }

    /// Emit `Ended` (once, via the session latch) and leave the playing
    /// phases. The single exit for both the immediate and the drained
    /// end-of-stream paths.
    fn finish_end_of_stream(&mut self) {
        self.drain = None;
        if let Some(loaded) = self.machine.loaded_mut()
            && !loaded.has_ended()
        {
            loaded.mark_ended();
            self.sinks.emit(PlayerEvent::Ended);
        }
        self.machine.end_of_stream();
    }

    /// While `Buffering`: poll the source's readiness and morph back to
    /// `Playing` once data has arrived **and** the buffered cushion has
    /// refilled to [`Worker::min_buffer`] (see
    /// [`Worker::buffered_ahead_sufficient`]) — resuming on the first
    /// byte would starve again a few hundred milliseconds later. Also
    /// keeps the position observable live — the pump is parked, but the
    /// device keeps draining its ring, so the audible position keeps
    /// advancing until the ring runs dry.
    fn poll_refill(&mut self) {
        // Every buffering tick refreshes the buffered-window mirror —
        // this is the state where it visibly grows.
        self.refresh_buffered();
        let live = self
            .machine
            .loaded_mut()
            .and_then(|loaded| loaded.output_position());
        if let Some(pos) = live {
            self.shared.set_position(pos);
        }
        let readiness = self
            .machine
            .loaded_mut()
            .map(|loaded| loaded.readiness());
        match readiness {
            Some(crate::Readiness::Ready) => {
                if self.buffered_ahead_sufficient() {
                    self.machine.buffer_refilled();
                }
            }
            // The source is in its terminal-error state (readiness is
            // the only window the worker has onto it from `Buffering`):
            // give the pump one tick so the hard error surfaces through
            // the normal decode path and the worker parks, instead of
            // buffering forever on a dead source (the screen-off network
            // death, exactly).
            Some(crate::Readiness::Failed) => self.pump_buffering_once(),
            _ => {}
        }
    }

    /// One decode attempt from `Buffering` when the source reports
    /// [`Readiness::Failed`] — the terminal-error state. The attempt
    /// surfaces the sticky error through the normal decode path (the
    /// read returns it immediately; nothing decodes ahead), and the
    /// worker parks. Without this the playing pump never runs from
    /// `Buffering` and the player would buffer forever on a dead
    /// source (the screen-off network death, exactly).
    fn pump_buffering_once(&mut self) {
        let outcome = {
            let Some(loaded) = self.machine.loaded_mut() else {
                return;
            };
            // A scratch emit clock: position events are a Playing-tick
            // concern; this attempt only decodes (or surfaces the error).
            loaded.pump(&mut Instant::now(), Duration::MAX)
        };
        match outcome {
            PumpOutcome::Frame { position, .. } => self.shared.set_position(position),
            PumpOutcome::NeedsData => {}
            PumpOutcome::EndOfStream => {
                // A dead source that reports EOF (or a stream whose tail
                // drained below the threshold at the true end): end the
                // track instead of buffering forever.
                self.finish_end_of_stream();
            }
            PumpOutcome::Skipped(err) => {
                if let Some(e) = err {
                    if let CantodeError::Source(msg) = &e {
                        // Nothing is sounding in `Buffering` — park
                        // immediately (no drain).
                        self.shared.set_source_error(Some(msg.clone()));
                        self.machine.pause();
                    }
                    self.report_source_error(e);
                }
            }
        }
    }

    /// Whether the live session's buffered cushion has reached the
    /// configured [`PlayerConfig::min_buffer_duration`].
    ///
    /// The cushion is *total unheard-but-buffered media time*: the
    /// source's window frontier (mapped onto media time by linear
    /// interpolation — `buffered_media_time`) minus the audible
    /// position, so decoded-but-unplayed audio in the sink ring counts
    /// alongside the undecoded window.
    ///
    /// Returns `true` (no gating) whenever the threshold can't be
    /// meaningfully evaluated — a zero threshold, no live session, a
    /// source without a buffered window (memory / local files), an
    /// unknown duration or total length — or when the window already
    /// covers the rest of the track (`end >= total`), so the tail of a
    /// short track never wedges in `Buffering`.
    ///
    /// **Config constraint**: the threshold must stay below what the
    /// source's readahead window can hold in media time; a source that
    /// can never buffer this much ahead stays parked until its window
    /// reaches the stream end.
    fn buffered_ahead_sufficient(&mut self) -> bool {
        if self.min_buffer.is_zero() {
            return true;
        }
        let Some(loaded) = self.machine.loaded_mut() else {
            return true;
        };
        let Some(range) = loaded.buffered_range() else {
            return true;
        };
        if range.total.is_some_and(|total| range.end >= total) {
            return true;
        }
        // `loaded` borrows `self.machine`; `self.shared` is a disjoint
        // field, so this reads fine alongside it.
        let Some(duration) = self.shared.duration() else {
            return true;
        };
        let Some(frontier) = buffered_media_time(&range, duration) else {
            return true;
        };
        // The audible position: the sink's realtime output clock when
        // tracked, else the shared position mirror (the decode frontier
        // for untracked sinks — tests).
        let audible = loaded
            .output_position()
            .unwrap_or_else(|| self.shared.position());
        frontier.saturating_sub(audible) >= self.min_buffer
    }

    /// Surface a source/decode error as `PlayerEvent::Error` — once per
    /// episode (a failing source errors on every pump; the UI doesn't
    /// want a stream of identical events). Cleared by a successful seek.
    fn report_source_error(&mut self, e: CantodeError) {
        if self.error_latched {
            return;
        }
        self.error_latched = true;
        self.sinks.emit(PlayerEvent::Error(e));
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the worker's operations (`do_seek`, `do_stop`)
    //! against a stub-backed machine, using the doubles from
    //! `crate::player::stubs`. No audio device needed.

    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::AudioFormat;
    use crate::CantodeError;
    use crate::decoder::DecodedFrame;
    use crate::player::stubs::{
        FrameDecoder, StubDecoder, StubFactory, loaded_session, loaded_session_with,
    };
    use crate::state::PlayerState;

    fn worker_with(machine: Machine, shared: Arc<SharedStatus>) -> Worker {
        let (_tx, rx) = mpsc::channel();
        Worker {
            machine,
            decoder_factory: Arc::new(StubFactory),
            cmd_rx: rx,
            sink_factory: Arc::new(|| Err(CantodeError::Internal("stub factory".into()))),
            shared,
            sinks: EventSinks::default(),
            error_latched: false,
            drain: None,
            min_buffer: Duration::ZERO,
        }
    }

    #[test]
    fn seek_requires_a_session() {
        let shared = Arc::new(SharedStatus::new());
        let mut worker = worker_with(
            Machine::new(Arc::clone(&shared), EventSinks::default()),
            shared,
        );
        let err = worker.do_seek(Duration::from_secs(1)).unwrap_err();
        assert!(matches!(err, CantodeError::InvalidState(_)));
    }

    #[test]
    fn seek_flushes_the_sink_and_clears_the_ended_latch() {
        let (loaded, fx) = loaded_session(2, 2);
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );

        // Simulate an already-ended session, then seek back into it.
        worker.machine.loaded_mut().unwrap().mark_ended();
        let actual = worker.do_seek(Duration::from_secs(5)).unwrap();

        assert_eq!(actual, Duration::from_secs(5));
        assert!(fx.log.recorded("flush"));
        assert!(!worker.machine.loaded_mut().unwrap().has_ended());
        assert_eq!(fx.shared.position(), Duration::from_secs(5));
    }

    #[test]
    fn stop_tears_the_session_down_and_resets_observables() {
        let (loaded, fx) = loaded_session(2, 2);
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        fx.shared.set_position(Duration::from_secs(9));
        fx.shared.set_duration(Some(Duration::from_secs(9)));

        worker.do_stop();

        assert_eq!(worker.machine.state(), PlayerState::Idle);
        assert_eq!(fx.shared.state(), PlayerState::Idle);
        assert!(fx.log.recorded("stop"));
        assert_eq!(fx.shared.position(), Duration::ZERO);
        assert_eq!(fx.shared.duration(), None);
    }

    #[test]
    fn eof_without_output_tracking_ends_immediately() {
        // Sinks that don't report their output position keep the
        // historical behavior: `Ended` at decode EOF.
        let (loaded, fx) = loaded_session(2, 2);
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        worker.machine.play();

        worker.pump_once(); // stub decoder: immediate EOF

        assert_eq!(worker.machine.state(), PlayerState::Ended);
        assert_eq!(fx.shared.state(), PlayerState::Ended);
    }

    #[test]
    fn hard_source_error_parks_on_paused_and_records_the_message() {
        // A hard source error is terminal: the worker parks on `Paused`
        // (position and session survive) and publishes the message for
        // the poll. A seek is the user-driven recovery — fresh source
        // epoch, error message cleared.
        let (loaded, fx) = loaded_session_with(
            StubDecoder {
                fmt: AudioFormat::new(2, 48_000),
                fail_once: Some(crate::CantodeError::Source("network died".into())),
                eof: true,
                buffered: None,
            },
            2,
            2,
        );
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        worker.machine.play();

        worker.pump_once(); // the stub's hard source error

        assert_eq!(worker.machine.state(), PlayerState::Paused);
        assert_eq!(fx.shared.state(), PlayerState::Paused);
        assert_eq!(fx.shared.source_error().as_deref(), Some("network died"));

        worker.do_seek(Duration::from_secs(1)).unwrap();
        assert_eq!(fx.shared.source_error(), None);
    }

    #[test]
    fn decode_errors_keep_skipping_without_parking() {
        // Decode failures are not terminal: the historical
        // skip-and-continue stands — no park, no poll-visible error.
        let (loaded, fx) = loaded_session_with(
            StubDecoder {
                fmt: AudioFormat::new(2, 48_000),
                fail_once: Some(crate::CantodeError::Decode("corrupt".into())),
                eof: true,
                buffered: None,
            },
            2,
            2,
        );
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        worker.machine.play();

        worker.pump_once();

        assert_eq!(worker.machine.state(), PlayerState::Playing);
        assert_eq!(fx.shared.source_error(), None);
    }

    #[test]
    fn eof_with_output_tracking_ends_only_after_the_tail_drains() {
        // The stub decoder yields one frame (ts 9 s, 480 frames @ 48 kHz
        // = 10 ms), then EOF. The tracking sink reports the instant-play
        // model, so the drain completes once its reported position
        // reaches the frame's end (9.01 s). Held mid-drain, the sink
        // still holds audio and the position sits 510 ms short — beyond
        // the settle tolerance — so the drain keeps waiting.
        let (loaded, fx) = loaded_session_with(
            FrameDecoder {
                frame: DecodedFrame {
                    data: vec![0.0; 2 * 480],
                    frames: 480,
                    timestamp: Duration::from_secs(9),
                },
                yielded: false,
            },
            2,
            2,
        );
        fx.enable_output_tracking();
        fx.set_undrained(Some(Duration::from_secs(1)));
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        worker.machine.play();

        worker.pump_once(); // decode + write the frame (position → 9.01 s)
        assert_eq!(worker.machine.state(), PlayerState::Playing);

        // Hold the tail mid-buffer: EOF arms the drain, no end yet.
        fx.set_output_position(Some(Duration::from_secs_f64(8.5)));
        worker.pump_once(); // EOF → drain armed
        assert_eq!(worker.machine.state(), PlayerState::Playing);
        worker.pump_once(); // frozen 510 ms short, ring holds audio — still waiting
        assert_eq!(worker.machine.state(), PlayerState::Playing);
        // The drain tick mirrors the live output position so the
        // progress glides to the end instead of freezing short.
        assert_eq!(fx.shared.position(), Duration::from_secs_f64(8.5));

        // The tail has sounded: 9.01 s ≥ target → Ended.
        fx.set_output_position(Some(Duration::from_millis(9_010)));
        worker.pump_once();
        assert_eq!(worker.machine.state(), PlayerState::Ended);
        assert!(worker.machine.loaded_mut().unwrap().has_ended());

        // Exactly once: another tick changes nothing.
        worker.pump_once();
        assert_eq!(worker.machine.state(), PlayerState::Ended);
    }

    #[test]
    fn drain_frozen_short_of_target_ends_once_the_ring_empties() {
        // The regression behind "position pinned at the end, then ~8 s
        // of dead air": a clock that freezes a hair short of the
        // ts-derived target used to wait out the whole DRAIN_MAX_STALL,
        // because the finish test had no tolerance. An empty ring means
        // everything decoded has sounded — end immediately regardless
        // of the remaining gap.
        let (loaded, fx) = loaded_session_with(
            FrameDecoder {
                frame: DecodedFrame {
                    data: vec![0.0; 2 * 480],
                    frames: 480,
                    timestamp: Duration::from_secs(9),
                },
                yielded: false,
            },
            2,
            2,
        );
        fx.enable_output_tracking();
        fx.set_undrained(Some(Duration::from_secs(1)));
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        worker.machine.play();

        worker.pump_once(); // decode + write → position 9.01 s
        fx.set_output_position(Some(Duration::from_secs_f64(8.5)));
        worker.pump_once(); // EOF → drain armed
        worker.pump_once(); // first drain tick: progress (last_pos starts at zero)
        worker.pump_once(); // frozen short, ring holds audio → stall armed, waiting
        assert_eq!(worker.machine.state(), PlayerState::Playing);

        // The device drained the ring: end NOW, not after the stall budget.
        fx.set_undrained(Some(Duration::ZERO));
        worker.pump_once();
        assert_eq!(worker.machine.state(), PlayerState::Ended);
        assert!(worker.machine.loaded_mut().unwrap().has_ended());
    }

    #[test]
    fn drain_settles_within_tolerance_of_the_target() {
        // A frozen clock within DRAIN_SETTLE_TOLERANCE of the target is
        // bookkeeping drift, not unheard audio: the drain settles on the
        // first frozen tick even though the sink still reports audio.
        let (loaded, fx) = loaded_session_with(
            FrameDecoder {
                frame: DecodedFrame {
                    data: vec![0.0; 2 * 480],
                    frames: 480,
                    timestamp: Duration::from_secs(9),
                },
                yielded: false,
            },
            2,
            2,
        );
        fx.enable_output_tracking();
        fx.set_undrained(Some(Duration::from_secs(1)));
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        worker.machine.play();

        worker.pump_once(); // decode + write → position 9.01 s
        fx.set_output_position(Some(Duration::from_millis(9_000))); // 10 ms short
        worker.pump_once(); // EOF → drain armed
        worker.pump_once(); // first drain tick: progress (last_pos starts at zero)
        worker.pump_once(); // frozen 10 ms short → settle immediately
        assert_eq!(worker.machine.state(), PlayerState::Ended);
    }

    #[test]
    fn drain_keeps_waiting_while_the_ring_still_holds_audio() {
        // A frozen clock with a real shortfall and audio still held is
        // the wedged-device case: the drain must NOT end early — the
        // stall budget (not tested here; it needs a real clock) stays
        // the backstop. Progress resumes the wait too.
        let (loaded, fx) = loaded_session_with(
            FrameDecoder {
                frame: DecodedFrame {
                    data: vec![0.0; 2 * 480],
                    frames: 480,
                    timestamp: Duration::from_secs(9),
                },
                yielded: false,
            },
            2,
            2,
        );
        fx.enable_output_tracking();
        fx.set_undrained(Some(Duration::from_secs(1)));
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        worker.machine.play();

        worker.pump_once(); // decode + write → position 9.01 s
        fx.set_output_position(Some(Duration::from_secs_f64(8.0))); // 1.01 s short
        worker.pump_once(); // EOF → drain armed
        worker.pump_once(); // progress tick
        worker.pump_once(); // frozen, ring holds audio → armed, waiting
        assert_eq!(worker.machine.state(), PlayerState::Playing);
    }

    #[test]
    fn pause_cancels_a_pending_drain_and_eof_re_arms_it() {
        let (loaded, fx) = loaded_session(2, 2);
        fx.enable_output_tracking();
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        worker.machine.play();
        worker.pump_once(); // EOF (no frames) → drain armed
        assert_eq!(worker.machine.state(), PlayerState::Playing);
        assert!(worker.drain.is_some());

        // User pauses mid-drain (through the command door, as production
        // does): the drain is cancelled, not stalled through it.
        worker.handle_command(Command::Pause);
        assert_eq!(worker.machine.state(), PlayerState::Paused);
        assert!(worker.drain.is_none());

        // Resume: the pump hits EOF again and re-arms the drain.
        worker.handle_command(Command::Play);
        assert_eq!(worker.machine.state(), PlayerState::Playing);
        worker.pump_once(); // EOF → drain re-armed
        assert_eq!(worker.machine.state(), PlayerState::Playing);
        worker.pump_once(); // drain completes instantly (target 0 = pos 0)
        assert_eq!(worker.machine.state(), PlayerState::Ended);
    }

    #[test]
    fn refresh_buffered_mirrors_the_session_range() {
        // A paused session whose source reports a window: one tick
        // publishes it into the shared observables.
        let range = crate::BufferedRange {
            start: 0,
            end: 1024,
            total: Some(4096),
        };
        let (loaded, fx) = loaded_session_with(
            StubDecoder {
                fmt: AudioFormat::new(2, 48_000),
                fail_once: None,
                eof: true,
                buffered: Some(range),
            },
            2,
            2,
        );
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        assert_eq!(fx.shared.buffered_range(), None);

        worker.refresh_buffered();

        assert_eq!(fx.shared.buffered_range(), Some(range));
    }

    #[test]
    fn paused_with_a_buffering_source_uses_the_slow_refresh_tick() {
        let range = crate::BufferedRange {
            start: 0,
            end: 1,
            total: Some(2),
        };
        let (loaded, fx) = loaded_session_with(
            StubDecoder {
                fmt: AudioFormat::new(2, 48_000),
                fail_once: None,
                eof: true,
                buffered: Some(range),
            },
            2,
            2,
        );
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        assert_eq!(worker.machine.state(), PlayerState::Paused);
        assert_eq!(worker.next_timeout(), BUFFERED_REFRESH_INTERVAL);

        // Playing keeps the fast decode tick.
        worker.machine.play();
        assert_eq!(worker.next_timeout(), Duration::from_millis(5));

        // And back to paused — still the slow tick.
        worker.machine.pause();
        assert_eq!(worker.next_timeout(), BUFFERED_REFRESH_INTERVAL);
    }

    #[test]
    fn paused_without_a_buffering_source_keeps_the_idle_park() {
        // Non-buffering sources (memory, local files) must not keep the
        // worker waking: the pure event-driven 1 h idle is preserved.
        let (loaded, fx) = loaded_session(2, 2); // buffered: None
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        assert_eq!(worker.machine.state(), PlayerState::Paused);
        assert_eq!(worker.next_timeout(), Duration::from_secs(60 * 60));
    }

    // ---- min-buffer refill gating ----

    /// A worker resting in `Buffering` on a session whose decoder
    /// reports `buffered` (a fixed window — enough for the gate logic;
    /// window growth is covered by `tests/buffered_source.rs`).
    fn buffering_worker(
        buffered: Option<crate::BufferedRange>,
        duration: Option<Duration>,
        position: Duration,
        min_buffer: Duration,
    ) -> Worker {
        let (loaded, fx) = loaded_session_with(
            StubDecoder {
                fmt: AudioFormat::new(2, 48_000),
                fail_once: None,
                eof: false,
                buffered,
            },
            2,
            2,
        );
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        fx.shared.set_duration(duration);
        fx.shared.set_position(position);
        worker.min_buffer = min_buffer;
        worker.machine.play();
        worker.machine.buffer_underrun();
        assert_eq!(worker.machine.state(), PlayerState::Buffering);
        worker
    }

    #[test]
    fn refill_waits_for_the_min_buffer_cushion() {
        // Window end at byte 100 of 1000 on a 10 s track = a 1 s
        // frontier; the audible position is 0 → a 1 s cushion, below
        // the 2 s threshold. The refill poll must keep parking.
        let mut worker = buffering_worker(
            Some(crate::BufferedRange {
                start: 0,
                end: 100,
                total: Some(1000),
            }),
            Some(Duration::from_secs(10)),
            Duration::ZERO,
            Duration::from_secs(2),
        );

        worker.poll_refill();

        assert_eq!(worker.machine.state(), PlayerState::Buffering);
    }

    #[test]
    fn refill_resumes_once_the_cushion_reaches_the_threshold() {
        // Byte 300 of 1000 on a 10 s track = a 3 s frontier at position
        // 0 → a 3 s cushion ≥ the 2 s threshold → morph to `Playing`.
        let mut worker = buffering_worker(
            Some(crate::BufferedRange {
                start: 0,
                end: 300,
                total: Some(1000),
            }),
            Some(Duration::from_secs(10)),
            Duration::ZERO,
            Duration::from_secs(2),
        );

        worker.poll_refill();

        assert_eq!(worker.machine.state(), PlayerState::Playing);
    }

    #[test]
    fn the_cushion_is_measured_from_the_audible_position() {
        // Same 3 s frontier, but 2 s of it has already sounded: the
        // cushion is 1 s, below the threshold — no resume yet. Decoded
        // audio still in the sink ring counts as cushion precisely so
        // this math (frontier − audible) matches what the listener
        // would hear before a second stall.
        let mut worker = buffering_worker(
            Some(crate::BufferedRange {
                start: 0,
                end: 300,
                total: Some(1000),
            }),
            Some(Duration::from_secs(10)),
            Duration::from_secs(2),
            Duration::from_secs(2),
        );

        worker.poll_refill();

        assert_eq!(worker.machine.state(), PlayerState::Buffering);
    }

    #[test]
    fn refill_resumes_at_the_end_of_the_stream_regardless_of_the_threshold() {
        // The window covers the whole rest of the track (end == total):
        // nothing more can arrive, so even a 5 s threshold on a 1 s
        // frontier must resume — short tails never wedge in `Buffering`.
        let mut worker = buffering_worker(
            Some(crate::BufferedRange {
                start: 900,
                end: 1000,
                total: Some(1000),
            }),
            Some(Duration::from_secs(10)),
            Duration::from_secs(9),
            Duration::from_secs(5),
        );

        worker.poll_refill();

        assert_eq!(worker.machine.state(), PlayerState::Playing);
    }

    #[test]
    fn refill_resumes_without_a_known_duration() {
        // No duration → the bytes→media-time mapping is impossible; the
        // gate degrades to readiness-only rather than parking forever.
        let mut worker = buffering_worker(
            Some(crate::BufferedRange {
                start: 0,
                end: 1,
                total: Some(1000),
            }),
            None,
            Duration::ZERO,
            Duration::from_secs(2),
        );

        worker.poll_refill();

        assert_eq!(worker.machine.state(), PlayerState::Playing);
    }

    #[test]
    fn zero_threshold_restores_readiness_only_refill() {
        // `Duration::ZERO` opts out of the gate entirely — the legacy
        // resume-on-any-data behavior the pre-threshold suite pins.
        let mut worker = buffering_worker(
            Some(crate::BufferedRange {
                start: 0,
                end: 1,
                total: Some(1000),
            }),
            Some(Duration::from_secs(10)),
            Duration::ZERO,
            Duration::ZERO,
        );

        worker.poll_refill();

        assert_eq!(worker.machine.state(), PlayerState::Playing);
    }
}
