---
name: Batch Plan Implementation
description: EXPLICIT USER REQUEST ONLY — run this skill solely when the user asks for it BY NAME (e.g. "use batch-plan-impl"); a bare "go"/"execute the plan", or the existence of an approved batch-format plan, does NOT authorize it (implement directly in the current session instead). Once explicitly invoked: execute a multi-phase/step plan unattended — the orchestrator first switches its own session's model (keeping build mode) to main.complex from models.jsonc, then creates one headless OpenCode session per task, each pinned to main.balance (a phase has one task, or several PARALLEL tasks when the plan splits it so), and dispatches their prompts, while a small watch-only subagent on main.balance polls every 60s (all sessions of the phase), relays prepared context notes between tasks when triggered, verifies the commits+push, and reports; after each phase a plan-alignment review subagent — pinned to main.balance, like every dispatched session — reviews the landed diff against the plan, and any misalignment or high-priority issue is dispatched back to the phase session as a fix round, the review→fix loop repeating until the review reports ALIGNED with no high-priority issues; continue phase by phase until the plan is done. No user interaction once legitimately invoked: decide autonomously, retry once, never delete sessions.
---

# Batch Plan Implementation

## Invocation gate — explicit user request ONLY

This skill runs ONLY when the user explicitly asks for it **by name**
(e.g. "use batch-plan-impl", "run it with batch-plan-impl"). It is
NEVER inferred from:

- a bare "go" / "execute the plan" / "run it" / "start";
- the existence of an approved plan, or a plan written in the batch
  format (run-log paths, phase/commit conventions);
- this session having authored, saved, or suggested the plan (or this
  skill) earlier in the conversation.

Any of those mean: implement the plan DIRECTLY in the current session,
phase by phase, visibly to the user — or ask the user how they want it
run. Do NOT fan out worker sessions. If you discover mid-batch that the
invocation was not explicit: interrupt any dispatched worker sessions,
leave the tree for review, and report.

The "no user interaction / decide autonomously" policy below applies
only AFTER this gate has passed.

---

Drive an existing multi-phase/step plan to completion — **unattended**. The
user is usually away, so there is NO confirmation step and NO asking questions:
follow the autonomous decision policy below, record every decision in a run
log, and leave a complete report.

For each phase **you** (the session running this skill) create one fresh
headless OpenCode session per task and dispatch its prompt. A phase has
ONE task by default, or SEVERAL tasks run in PARALLEL when the plan
splits it that way (encouraged — see "Writing parallel-friendly plans"
below). A **watch-only subagent** monitors every session of the phase
(60s polling each), relays context between tasks when triggered (4p),
and verifies the results. When a phase's sessions are done and pushed, a
**plan-alignment review subagent** — pinned to `main.balance`, like every
dispatched session — reviews the landed code against the
plan; misalignment or high-priority issues go back to the working session
as a fix round, and the review→fix loop repeats until the review reports
aligned with no high-priority issues (4d). Phases run strictly sequentially
— parallelism lives INSIDE a phase, never across phases.

## Division of labor

| Actor | Does | Never does |
| --- | --- | --- |
| You (orchestrator) | plan parsing, session create, prompt dispatch (phase + fix-round), subagent spawn, batch control, authoring relay notes (4p) | implement a phase itself, delete a session |
| Subagent (one per phase, small prompt) | poll 60s across ALL the phase's sessions, detect stall/timeout, relay orchestrator-authored context notes between task sessions when their trigger fires, verify commits + push + clean tree, extract summaries, report | create sessions, author task instructions or relay notes, edit files, commit, delete sessions |
| Review subagent (one per review round, `general` agent) | read-only plan-alignment review (4d): diff vs the phase's plan text, verdict + prioritized issue list; may read files and run builds/tests | edit files, commit, push, create sessions, message phase sessions |

## Rules

- **Never ask the user anything.** Decide autonomously per the policy below
  and record the decision in the run log.
- **Handle anything that comes up yourself** (stalls, missing pushes, flaky
  failures) per the decision policy — never wait for the user.
- **Never delete a session.** "Closing" a phase just means moving on to the
  next one — every session stays in the session list for later inspection.
- One phase = 1..N task sessions = ONE watch subagent for all of them.
  Never batch phases.
- **Parallel tasks must declare disjoint file scopes** (the plan states
  each task's scope; each task prompt repeats it). If two tasks must
  touch the same file, the plan serializes them as separate phases or
  splits at a clean file boundary — the orchestrator rejects fan-out
  with overlapping scopes.
- **Pushes serialize naturally**: each task commits only its own scope
  and pushes; on rejection it rebases ONLY its own commit and retries
  once. Sibling commits are expected cargo, never failures.
- The subagent prompt stays SMALL: session ids + a few context values.
  The long task text goes only into the dispatched session prompts,
  never into the subagent.
- Never implement a phase in this session. You orchestrate and verify only.
- Poll status every **60 seconds**. Do not use blocking waits.
- Models come from **`models.jsonc`** (repo root), never `opencode.jsonc`.
  There is ONE pair, no window: `main.complex` and `main.balance`. Re-resolve
  before EVERY session create or dispatch — `models.jsonc` can change
  mid-batch (see step 0 / step 3).
- Pin models explicitly on every dispatch: **the orchestrator (this
  session) runs `main.complex`; every DISPATCHED session — workers,
  plan-alignment reviewers, and watch subagents — pins `main.balance`**.
  Re-resolve both refs before every dispatch.
- **Every phase is gated by the plan-alignment review loop (4d)**: after the
  phase's sessions land and push, the review subagent compares the diff to
  the phase's plan text. Misalignment or ANY high-priority issue loops back
  to the working session as a fix round; the loop exits only on
  `VERDICT: ALIGNED` + `HIGH: none`.
- **Switch your session's MODEL to `main.complex` first**, **keeping the
  `build` agent/mode**, so all orchestration reasoning runs on it (see
  step 0).

## Autonomous decision policy

| Situation | Action |
| --- | --- |
| Invoked without the user explicitly asking for this skill BY NAME | Abort before touching anything: interrupt any dispatched worker sessions, leave the tree for review, report — then implement directly in-session or ask the user. |
| No plan found anywhere | Abort before touching anything; report where you looked. |
| Ambiguous phase boundaries/ordering | Best-effort split, record assumptions in the run log, proceed. |
| Dirty working tree | `git stash push --include-untracked -m "batch-plan-impl: auto-stash <date>"`, record in run log, proceed. |
| Phase FAILED (no commit / not pushed / dirty tree / outcome != succeeded) | Retry the phase ONCE with a fresh session and the same prompt. |
| PARALLEL: one task failed, siblings fine | Retry ONLY the failed task once (fresh session, same prompt + updated landed-state); siblings proceed untouched. |
| PARALLEL: a task's scope collides with a sibling's landed files | The orchestrator dispatches a reconcile prompt to the affected task (rebase onto sibling, adapt); if that fails once too, stop the batch. |
| PARALLEL: a task needs a sibling's landed facts mid-flight | The watcher fires the pre-arranged relay (4p); if none was arranged, the watcher reports and the orchestrator authors one. |
| Phase STALLED/TIMEOUT | Interrupt the session (`POST /api/session/$SID/interrupt`), then retry ONCE. |
| Review finds NOT_ALIGNED or high-priority issues | Dispatch a fix-round prompt (reviewer's findings VERBATIM) to the SAME phase session, re-watch with the new $SINCE, re-review; loop until `VERDICT: ALIGNED` + `HIGH: none` (4d). |
| Review loop makes no progress (fix round lands an empty diff / no new commit, or two consecutive rounds return an identical HIGH set) | Anti-deadlock guard — the loop's only forced exit besides success: treat as STALLED, retry the round ONCE; still stuck → stop the batch, record it. |
| Review subagent itself fails (no verdict / crashes / times out) | Retry the review ONCE with a fresh subagent; if it still fails, record the skipped gate and continue to the next phase, flagging the phase in the final report. |
| Phase fails after retry | **Stop the batch** — later phases likely depend on it. Leave everything for review. |
| Anything else unexpected | Choose the least destructive option, record it, keep going if safe. |

## Workflow

### 0. Resolve the model refs and switch this session's MODEL (keep build mode)

Models live in **`models.jsonc`** at the repo root — never `opencode.jsonc`.
There is one pair, no window: `main.complex` (the orchestrator — you) and
`main.balance` (every dispatched session: workers, reviewers, watchers).
Before any batch work,
switch **your own session's model** to `main.complex` so all orchestration
reasoning runs on it.

**Model only — the session's agent/mode stays `build`.** Never switch the
agent to `plan`: that is a different thing (the read-only plan mode) and would
make this session unable to run tools. `POST /api/session/{id}/model` changes
only the model, so the agent is untouched. The switch applies to subsequent
turns.

```sh
cd "$(git rev-parse --show-toplevel)"   # config + sessions are location-scoped

# Your own session id: the most recently updated session RIGHT NOW —
# this very turn is updating it:
opencode api get /api/session \
  | jq -r '.data | sort_by(.time.updated) | reverse | .[0] | .id'   # -> $SELF

# The model refs from models.jsonc (strip // comments, then parse):
MODELS=$(sed 's://.*$::' models.jsonc)
COMPLEX_REF=$(printf '%s' "$MODELS" | jq -r '.main.complex')    # the orchestrator (you)
BALANCE_REF=$(printf '%s' "$MODELS" | jq -r '.main.balance')    # workers + reviews + watchers

opencode api post /api/session/$SELF/model \
  --data "$(jq -n --arg ref "$COMPLEX_REF" \
    '{model:{providerID:($ref|split("/")[0]), id:($ref|split("/")[1])}}')"
```

Sanity-check `$SELF` against the session list if several sessions were touched
in the same second. If `models.jsonc` is missing/unreadable or the switch
fails, record it in the run log and continue on the current model — do not
block the batch on this.

### 1. Collect the plan

Identify the plan to execute, in this order:

1. The plan most recently produced in **this conversation** (e.g. by a plan
   agent or a previous discussion).
2. The newest obvious plan document (`rfc/`, `docs/`, `plans/`, `*.plan.md`,
   TODO files) — pick the most recently modified if several.

Extract an ordered list of phases/steps. For each, capture:

- `n` — ordinal
- `title` — short name
- `detail` — the FULL text of that phase/step from the plan, verbatim
  (acceptance criteria, file paths, spec excerpts — everything)

If none is found, abort and report. If boundaries/order are ambiguous, split
best-effort and record your assumptions.

### 2. Run log

Create `/tmp/opencode/ease-batch-plan-impl/run.md` and record, as you go: the plan
source, the parsed phase list, every autonomous decision (with reason), each
session id, and each phase result. The final report is generated from this.

### 3. Preconditions

```sh
git status --porcelain          # if dirty: auto-stash (see policy), record it
git rev-parse HEAD              # record as $BASE
pwd                             # record as $PROJECT_DIR (absolute)
mkdir -p /tmp/opencode/ease-batch-plan-impl   # scratch for payload files + run log
```

Resolve the model refs from `models.jsonc` — re-run the resolution from
step 0 (`models.jsonc` can change mid-batch). Every dispatched session —
workers, reviewers, watchers — pins `$BALANCE_REF`; only this orchestrator
session runs `$COMPLEX_REF`:

```sh
# $BALANCE_REF from the step-0 resolution (re-checked NOW) — EVERY dispatch:
#   $MODEL_JSON = {"providerID":"${BALANCE_REF%%/*}","id":"${BALANCE_REF##*/}"}
#   $MODEL_REF  = $BALANCE_REF
# (the orchestrator's own session already runs $COMPLEX_REF — step 0)
```

### 4. Per phase (loop n = 1..N)

#### a. Create the session (you)

```sh
opencode api post /api/session \
  --data "$(jq -n --arg t "Phase <n>: <title>" --arg d "$PROJECT_DIR" \
            --arg m "$MODEL_JSON" \
            '{title:$t, location:{directory:$d}, model:($m|fromjson)}')"
```

Extract `$SID` via `jq '.data.id'`. `location.directory` is REQUIRED — without
it the session lands in the wrong directory. `model` pins `main.balance`
explicitly (re-resolved per dispatch); do not rely on inheritance.

#### b. Dispatch the phase prompt (you)

Write the phase prompt to a payload file (never interpolate raw text into
`--data`), then dispatch and record the admission timestamp:

```sh
# phase<n>.txt content:
#   You are implementing phase <n> of an approved plan in this repository.
#
#   <detail>
#
#   Rules:
#   - Implement ONLY this phase. Do not start or anticipate other phases.
#   - Work within this repo. Keep changes minimal and consistent with existing code.
#   - When done, commit ALL changes with subject: "phase(<n>): <title>"
#     (plus a short body listing what was done).
#   - Then PUSH to the remote: git push (add -u <remote> <branch> if the
#     branch has no upstream yet). A phase is only done once pushed.
#   - Finish with a summary: what changed, files touched, test/build results.

jq -n --rawfile text /tmp/opencode/ease-batch-plan-impl/phase<n>.txt '{text:$text}' \
  > /tmp/opencode/ease-batch-plan-impl/phase<n>.json

opencode api post /api/session/$SID/prompt \
  --data "$(cat /tmp/opencode/ease-batch-plan-impl/phase<n>.json)"
# record .data.time.created as $SINCE (epoch ms)
```

#### c. Spawn the watch subagent (you)

One subagent (subagent tool, `general` agent, foreground — phases are
sequential), pinned to the balance model (`model: "$BALANCE_REF"` —
`main.balance` — in the subagent tool call). Fill only these placeholders:
`$PROJECT_DIR`, `$SID`, `$SINCE` (number), `$BASE`, `<n>`. Send exactly this
small prompt:

```text
Watch worker for a headless OpenCode session. Observe and report ONLY — never
create sessions, send prompts, edit files, or change git state.

Context:
- Project dir: $PROJECT_DIR
- Session id: $SID   (prompt already dispatched — do not touch it)
- Prompt admitted at epoch ms: $SINCE
- Base commit: $BASE ; expected commit subject prefix: "phase(<n>):"

WATCH — repeat until finished (each step is its own shell call):
  sleep 60
  opencode api session.message.list \
    --param sessionID="$SID" --param order=desc --param limit=1 \
    | jq -e '.data[0].type=="idle" and .data[0].time.created > $SINCE' >/dev/null
  exit 0 => loop finished (DONE)
- Newest message `idle` after $SINCE means the agent loop finished.
- Do NOT use /api/session/active — it does not track headless sessions.
- STALLED if the newest message id stops changing for 20 consecutive polls
  while not idle.   TIMEOUT after 240 polls (~4h) with no idle.

ON DONE, collect:
1. Outcome:
   opencode api session.message.list \
     --param sessionID="$SID" --param order=desc --param limit=5 \
     | jq -r '.data[] | select(.type=="idle") | .outcome'     # want: succeeded
2. Verification:
   git -C $PROJECT_DIR log --oneline $BASE..HEAD
   git -C $PROJECT_DIR status -sb        # first line must NOT contain [ahead]
   git -C $PROJECT_DIR status --porcelain
3. Final assistant summary:
   opencode api session.message.list \
     --param sessionID="$SID" --param order=desc --param limit=15 \
     | jq -r '[.data[] | select(.type=="assistant")][0]
              | [.content[] | select(.type=="text") | .text] | join("\n")'

RESPOND with exactly this and nothing else:
STATUS: DONE | STALLED | TIMEOUT
OUTCOME: <idle outcome or "n/a">
COMMITS: <hashes + subjects, or "none">
PUSHED: <yes/no — no "[ahead" in status -sb>
DIRTY: <yes/no>
SUMMARY: <final assistant text, max ~30 lines>
```

#### d. Plan-alignment review loop (review subagent — YOUR model)

Once the watcher reports DONE and the mechanical gates hold (commit, pushed,
clean tree), dispatch a **review subagent** (subagent tool, `general` agent,
foreground) pinned to `$BALANCE_REF` — `main.balance`, like every dispatched
session. The reviewer is read-only over git state and
the sessions: it may read files, run `git diff/log/show`, and run
builds/tests, but never edits, commits, pushes, or messages any session.

Build the payload with `jq -n --rawfile` into `review<n>-r<k>.txt` (`k` =
review round, starting at 1). Fill: `$PROJECT_DIR`, `$BASE`, `<n>`,
`<title>`, the phase's FULL `detail` from the plan (verbatim), and for
rounds k>1 the prior findings. Send exactly this:

```text
You are a plan-alignment reviewer for phase <n> of an approved plan. Observe
and report ONLY — never edit files, commit, push, or message any session.

Context:
- Project dir: $PROJECT_DIR
- Phase diff to review: git -C $PROJECT_DIR diff $BASE..HEAD
  (commit list: git -C $PROJECT_DIR log --oneline $BASE..HEAD)
- Task scope (parallel phases only): <declared file scope, or "whole phase">
- Phase <n> — "<title>" — FULL plan text (the contract):
  <detail>
- Prior review rounds (rounds 2+): <prior HIGH findings, or "none — first round">

REVIEW (read-only):
1. Read the diff and every file it touches; check the implementation against
   the plan text ABOVE — every acceptance criterion, path, name, and spec
   point must be honored or consciously superseded by the plan's own text.
2. Run the project's build/tests if the diff can affect them.
3. Classify every problem:
   - HIGH: plan misalignment (missing, contradicted, or silently reinterpreted
     phase text) or any correctness, data-loss, security, or
     breaking-regression issue.
   - MINOR: style, naming, nice-to-haves — record but do NOT gate on these.

RESPOND with exactly this and nothing else:
VERDICT: ALIGNED | NOT_ALIGNED
HIGH: <numbered list, each with file:line and a one-line fix hint, or "none">
MINOR: <short list or "none">
SUMMARY: <max ~15 lines>
```

**Loop** — the phase passes review ONLY on `VERDICT: ALIGNED` **and**
`HIGH: none`. Otherwise (round k did not pass):

1. Write `fix<n>-r<k>.txt` — a fix-round prompt for the SAME working session
   `$SID`: state that the phase is implemented but the plan-alignment review
   found gaps, embed the reviewer's `VERDICT` + `HIGH` list **verbatim**
   (MINOR items may be attached as optional), and instruct: resolve every
   HIGH item, stay within the phase's scope, commit as
   `phase(<n>): review fixes (round <k>)` (same body/push rules as the
   original phase prompt).
2. Dispatch it via `POST /api/session/$SID/prompt`, record the new `$SINCE`,
   spawn a fresh watcher (step 4c — same expected prefix `phase(<n):`), and
   when it reports DONE re-run this review step as round k+1 with the
   updated prior-findings history.
3. Repeat — the loop has no iteration cap and exits ONLY on
   `VERDICT: ALIGNED` + `HIGH: none`. One anti-deadlock guard: if a fix
   round produces an empty diff / no new commit, or two consecutive rounds
   return an identical `HIGH` set, treat the phase as STALLED (policy
   table) — retry the round once, else stop the batch and record.

In a PARALLEL phase (4p): run ONE review subagent for the whole phase once
ALL tasks are green (`$BASE..HEAD` covers every task), listing each task's
declared scope in the prompt; group the `HIGH` findings by scope and run a
fix round on EACH affected task's session (new `$SINCE` + fresh watcher per
session), then re-review. The phase's review closes when NO scope has open
HIGH items.

#### e. Review and continue (you)

Phase succeeded only when ALL hold: `STATUS: DONE`, `OUTCOME: succeeded`,
at least one commit, `PUSHED: yes`, clean tree, **and the review loop of 4d
has closed** (`VERDICT: ALIGNED`, `HIGH: none`). Then leave the sessions
as-is and start the next phase (back to 4a).

Otherwise (FAILED / STALLED / TIMEOUT): retry ONCE per policy (fresh session,
same prompt; interrupt first if stalled). Retry succeeds → continue. Retry
fails too → **stop the batch** and go to the final report. In a parallel
phase, retry only the failed TASK; the phase completes when ALL tasks are
DONE + succeeded + pushed + clean.

### 4p. Parallel task phases (a phase with several tasks)

When the plan splits a phase into tasks T1..Tn:

1. **Fan out**: create one session per task (4a, same pinning), each prompt
   carrying: the task's full text, its DECLARED FILE SCOPE, the list of
   sibling scopes (so a task recognizes expected foreign commits), and the
   phase's shared gates. Dispatch all prompts.
2. **One watcher for all**: spawn a single subagent that polls every task
   session each cycle (sleep 60 → check each newest message) and completes
   when ALL are idle after their SINCE timestamps. Its report is
   per-task: STATUS/OUTCOME/COMMITS/PUSHED/DIRTY/SUMMARY.
3. **Relay protocol (the information swap)**: at spawn time the
   orchestrator may arm the watcher with relay notes — each note =
   trigger (task X finished) + payload (an ORCHESTRATOR-AUTHORED context
   note for task Y, e.g. the names/numbers X was to land). The watcher
   forwards the payload verbatim to Y's session via the prompt API the
   moment the trigger fires. The watcher never authors note content —
   it is a courier, not an instructor. If no note was armed and Y needs
   X's facts, the watcher says so in its report and the orchestrator
   authors the follow-up.
4. **Completion**: all tasks green → the whole-phase review loop of 4d (one
   reviewer on `$BALANCE_REF`, fix rounds routed per scope) → next phase. A
   failed task retries alone (policy table). Sibling pushes may land
   mid-flight — expected.

### Writing parallel-friendly plans

When authoring or amending a plan, PREFER phases split into independent
parallel tasks — independence first:

- measure vs implement vs document split naturally (evidence phases,
  README/report tasks, bench rows vs engine code);
- disjoint modules/files split naturally (a host crate vs a rut pkg vs
  an rfc; a new workload vs engine internals);
- tasks must not need each other's OUTPUTS mid-flight — if B consumes
  what A lands, that is two phases, not two tasks;
- when a task plausibly needs a sibling's FACTS (names, pinned numbers,
  landed interfaces), the plan names the fact and the orchestrator bakes
  it into a relay note (4p) instead of coupling the tasks;
- 2–4 tasks per phase is the sweet spot; more needs explicit
  justification in the plan.

### 5. Final report

After the last phase (or on abort), summarize from the run log:

```
Batch plan complete: <k>/<N> phases done (base <BASE> -> HEAD <short hash>)
  1. <title>  ✓ <commit> pushed, review passed (r<N> rounds)   ses_xxx
  2. <title>  ✗ STALLED in review round 2 (retried once, failed)   ses_yyy — left untouched
Overall: git diff --stat <BASE>..HEAD
Autonomous decisions: <auto-stash hash, retry counts, review-round counts, skipped gates, assumptions made>
All sessions were kept (never deleted) — resume any of them from the session list.
```

## Quick command reference

| Action | Actor | Command |
| --- | --- | --- |
| Switch own model to plan model | you | `opencode api post /api/session/$SELF/model --data '{"model":{"providerID":"<p>","id":"<m>"}}'` (agent stays `build`) |
| Resolve model refs (step 0/3) | you | `sed 's://.*$::' models.jsonc \| jq -r '.main.complex, .main.balance'` |
| Create session (pinned model) | you | `opencode api post /api/session --data "$(jq -n --arg t "..." --arg d "$PROJECT_DIR" --arg m "$MODEL_JSON" '{title:$t,location:{directory:$d},model:($m\|fromjson)}')"` |
| Dispatch prompt (phase OR fix round) | you | `opencode api post /api/session/$SID/prompt --data "$(cat payload.json)"` |
| Review subagent model | you | pin `model: "$BALANCE_REF"` in the subagent tool call — `main.balance`, like every dispatched session |
| Poll newest message | subagent | `opencode api session.message.list --param sessionID=$SID --param order=desc --param limit=1` |
| Done when | subagent | newest msg `type=="idle"` and `time.created > $SINCE` |
| Verify | subagent | `git log --oneline $BASE..HEAD` + `status -sb` (no `[ahead`) + `status --porcelain` + idle `outcome` |
| Final summary text | subagent | list limit=15 → first `assistant` msg → join `content[]` where `type=="text"` |
| Interrupt stalled session | you | `opencode api post /api/session/$SID/interrupt` |

## Notes

- This project's `opencode.jsonc` allows all permissions
  (`"action": "*"` / `"effect": "allow"`), so phase sessions never pause on
  permission asks — no permission handling is needed anywhere in the batch.
- Models come from `models.jsonc` at the repo root — one pair, no window:
  the orchestrator (this session) runs `main.complex` (orchestration
  reasoning); every dispatched session — workers, plan-alignment review
  subagents (4d), and watch subagents — runs `main.balance`. Re-resolve
  before every dispatch (`models.jsonc` can change mid-batch).
- Payload JSON must be built with `jq -n --rawfile` / `jq -n --arg` — never
  string-interpolated (phase text contains quotes and newlines).

#### Shared-index hazard (one tree, one git index)

Parallel task sessions share ONE working tree and therefore ONE git
index. A task that commits staged state can sweep a sibling's staged
files into its own commit. Rules:

- Stage by EXPLICIT PATH only (`git add <own files>`), never `git add .`
  / `git add -A` / bare `git commit -a`.
- Before committing, a task verifies its staged set contains ONLY its
  declared scope; discovering foreign paths staged is expected (a
  sibling is mid-flight) — commit around them, never include them.
- Repair protocol if a commit nevertheless swept sibling files AND is
  still unpushed: split the commit (`git reset HEAD^ -- <foreign paths>`
  then re-commit own paths), rebase only your own commit onto the
  sibling's pushed version when it lands, content-verify, then push.
  If already pushed: report to the orchestrator immediately — the
  watcher flags it as DIRTY/failure for the affected sibling to resolve.
