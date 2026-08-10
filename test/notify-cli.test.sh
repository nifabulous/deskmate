#!/usr/bin/env bash
# Regression tests for bin/deskmate-notify.
#
#   test/notify-cli.test.sh
#
# The tool exists to tell you a job finished, so the failure that matters most
# is the one where it says it worked and nothing happened. Two ways that used to
# happen, both covered here:
#
#   1. curl exits 0 on an HTTP 400, so a rejected event reported success.
#   2. A raw newline in a title is not legal inside a JSON string, so the no-jq
#      fallback built a body the server refused — and, per 1, said nothing.
#
# Runs against throwaway servers on a spare port. Never touches a real deskmate.

set -u

CLI="$(cd "$(dirname "$0")/.." && pwd)/bin/deskmate-notify"
PORT=8997
failures=0

check() {
  local name="$1" actual="$2" expected="$3"
  if [ "$actual" = "$expected" ]; then
    echo "ok   $name"
  else
    echo "FAIL $name — got '$actual', want '$expected'"
    failures=$((failures + 1))
  fi
}

# A server that accepts anything, recording what it received.
start_server() {
  local status="$1" out="$2"
  python3 - "$PORT" "$status" "$out" <<'PY' &
import sys, json
from http.server import BaseHTTPRequestHandler, HTTPServer
port, status, out = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
class H(BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        with open(out, "wb") as f:
            f.write(body)
        self.send_response(status); self.send_header("Content-Type", "application/json")
        self.end_headers(); self.wfile.write(b'{"ok":true}')
    def log_message(self, *a): pass
HTTPServer(("127.0.0.1", port), H).serve_forever()
PY
  SERVER_PID=$!
  for _ in $(seq 1 40); do
    if curl -s -o /dev/null -m 1 "http://127.0.0.1:$PORT/" 2>/dev/null; then break; fi
    sleep 0.1
  done
  sleep 0.3
}
stop_server() { kill "$SERVER_PID" 2>/dev/null; wait "$SERVER_PID" 2>/dev/null; }

BODY=$(mktemp)

# 1. Nothing listening at all: the message should say so, and the exit non-zero.
out=$(DESKMATE_PORT=$PORT "$CLI" "no server" 2>&1); rc=$?
check "exits non-zero when deskmate is not running" "$rc" "1"
case "$out" in *"not reachable"*) r=yes ;; *) r="no: $out" ;; esac
check "and says it could not be reached" "$r" "yes"

# 2. Server rejects the event. This is the one that used to report success.
start_server 400 "$BODY"
out=$(DESKMATE_PORT=$PORT "$CLI" "rejected" 2>&1); rc=$?
stop_server
check "exits non-zero when the event is rejected" "$rc" "1"
case "$out" in *"rejected the event"*) r=yes ;; *) r="no: $out" ;; esac
check "and distinguishes rejected from unreachable" "$r" "yes"

# 3. A title spanning two lines must still produce valid JSON, with or without
#    jq, and must round-trip intact rather than being silently mangled.
# jq lives in /usr/bin on macOS, so trimming PATH does not hide it — that made
# an earlier version of this test run the jq path twice and prove nothing. Build
# a PATH holding only what the script needs, with jq deliberately absent.
NOJQ=$(mktemp -d)
# bash included because the shebang is `env bash`, which resolves via PATH.
for b in bash curl awk; do ln -s "$(command -v "$b")" "$NOJQ/$b"; done
command -v jq >/dev/null 2>&1 && [ ! -e "$NOJQ/jq" ] || { echo "FAIL: could not hide jq"; exit 1; }

for mode in with-jq without-jq; do
  : > "$BODY"          # or the next check passes on the previous run's body
  start_server 200 "$BODY"
  if [ "$mode" = "without-jq" ]; then
    out=$(PATH="$NOJQ" DESKMATE_PORT=$PORT "$CLI" "line one
line two	tabbed" "detail \"quoted\" and \\ backslash" 2>&1); rc=$?
  else
    out=$(DESKMATE_PORT=$PORT "$CLI" "line one
line two	tabbed" "detail \"quoted\" and \\ backslash" 2>&1); rc=$?
  fi
  stop_server
  check "$mode: multi-line title exits zero" "$rc" "0"
  parsed=$(python3 -c "
import json,sys
d=json.load(open('$BODY'))
print(d['title'] == 'line one\nline two\ttabbed' and d['detail'] == 'detail \"quoted\" and \\\\ backslash')
" 2>/dev/null)
  check "$mode: title and detail survive the round trip" "$parsed" "True"
done

rm -rf "$BODY" "$NOJQ"
echo
if [ "$failures" -eq 0 ]; then echo "all passing"; else echo "$failures failing"; fi
exit $((failures > 0))
