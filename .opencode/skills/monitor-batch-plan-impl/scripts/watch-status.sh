#!/bin/bash
# watch-status.sh — read-only liveness report for a watched OpenCode session
# (the monitor-batch-plan-impl wake-up check).
#
# Usage: watch-status.sh <targetSessionID> [projectDir]
#
# Prints: the target's state, its newest assistant text, every session active
# in the last 40 min (TRUE liveness = newest message time, NOT the session
# table), running-tool-call hints, pending question forms, and git state.
set -u
SID="${1:?usage: watch-status.sh <targetSessionID> [projectDir]}"
DIR="${2:-$PWD}"
DB="${OPENCODE_DB:-$HOME/.local/share/opencode/opencode.db}"

python3 - "$SID" "$DB" <<'PY'
import sqlite3, sys, time, json

sid, db = sys.argv[1], sys.argv[2]
con = sqlite3.connect(f'file:{db}?mode=ro', uri=True)  # read-only: WAL-safe
cur = con.cursor()
now = int(time.time() * 1000)
age = lambda ms: round((now - ms) / 60000, 1) if ms else -1
fmt = lambda ms: time.strftime('%H:%M:%S', time.localtime(ms / 1000))

def model_ref(v):
    try:
        m = json.loads(v)
        ref = f"{m.get('providerID', '?')}/{m.get('id', '?')}"
        return f"{ref}#{m['variant']}" if m.get('variant') else ref
    except Exception:
        return '?'

row = cur.execute(
    'SELECT title, time_updated, time_idle, idle_outcome, model '
    'FROM session_v2 WHERE id=?', (sid,)).fetchone()
if not row:
    print(f'TARGET {sid}: NOT FOUND in {db}')
    sys.exit(1)
title, upd, idle, outcome, model = row
print(f"TARGET {sid} '{(title or '')[:60]}' last_activity={fmt(upd)} "
      f"({age(upd)}min ago) idle={fmt(idle) if idle else '-'} outcome={outcome} "
      f"model={model_ref(model)}")

def latest_assistant_text(s):
    for (data,) in cur.execute(
            "SELECT data FROM session_message "
            "WHERE session_id=? AND type='assistant' ORDER BY seq DESC LIMIT 3",
            (s,)):
        d = json.loads(data)
        for c in d.get('content', []):
            if c.get('type') == 'text' and c.get('text', '').strip():
                return d.get('time', {}).get('created'), c['text']
    return None, None

t, txt = latest_assistant_text(sid)
if txt:
    first = txt[:220].split('\n')[0]
    print(f"LAST_ASSISTANT({fmt(t)}, {age(t)}min ago): {first}")

print('\nACTIVE, last 40 min (liveness = newest MESSAGE time; the session '
      'table lags during long tool calls):')
rows = cur.execute("""
    SELECT s.id, s.title, MAX(m.time_created), s.model
    FROM session_v2 s JOIN session_message m ON m.session_id = s.id
    WHERE s.time_archived IS NULL
    GROUP BY s.id
    HAVING MAX(m.time_created) > ?
    ORDER BY MAX(m.time_created) DESC
    LIMIT 12
""", (now - 40 * 60000,)).fetchall()
if not rows:
    print('  (none — everything is quiet)')
for sid2, title2, tc, mdl in rows:
    newest = cur.execute(
        'SELECT type FROM session_message WHERE session_id=? '
        'ORDER BY seq DESC LIMIT 1', (sid2,)).fetchone()
    mark = '  <== TARGET' if sid2 == sid else ''
    print(f"  {sid2} '{(title2 or '')[:44]}' last_msg={age(tc)}min "
          f"type={newest[0] if newest else '?'} model={model_ref(mdl)}{mark}")

# Stall hints: newest assistant message of an active session still inside a
# tool call. Quiet != stalled until ~45 min AND nothing is being written.
for sid2, title2, tc, _mdl in rows:
    for (data,) in cur.execute(
            "SELECT data FROM session_message "
            "WHERE session_id=? AND type='assistant' ORDER BY seq DESC LIMIT 1",
            (sid2,)):
        d = json.loads(data)
        for c in d.get('content', []):
            if c.get('type') == 'tool':
                st = c.get('state', {})
                if st.get('status') in ('running', 'pending'):
                    tstart = d.get('time', {}).get('created')
                    print(f"  TOOL-RUNNING {sid2}: {c.get('name')} since "
                          f"{fmt(tstart)} ({age(tstart)}min) — stall only if "
                          f">~45min AND no output files growing")
                break
PY

echo
echo "--- pending question forms on $SID (empty list = none) ---"
(cd "$DIR" && opencode api get "/api/session/$SID/form" 2>/dev/null) \
  | head -c 400
echo

echo "--- git ---"
git -C "$DIR" log --oneline -5 2>/dev/null
git -C "$DIR" status -sb 2>/dev/null | head -1
