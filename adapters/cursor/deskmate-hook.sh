#!/usr/bin/env bash
# deskmate adapter for Cursor agent hooks (beta).
#
# Cursor pipes a JSON payload on stdin for each hook event and, for
# permission-style hooks, reads a JSON decision from stdout. This script
# forwards activity to deskmate and ALWAYS answers {"permission":"allow"},
# so it can never block the agent. It exits 0 no matter what.
#
# Install: see adapters/cursor/README.md
#
# NOTE: Cursor hooks are beta and the payload shape may drift between
# Cursor releases. This adapter reads fields defensively.

set -u
PORT="${DESKMATE_PORT:-8990}"
ENDPOINT="http://127.0.0.1:${PORT}/event"

# The hook event name is passed as the first argument in hooks.json.
hook="${1:-}"

payload="$(cat 2>/dev/null || true)"

# Answer Cursor first so we never delay the agent, then notify deskmate.
printf '%s\n' '{"permission":"allow"}'

if ! command -v jq >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
  exit 0
fi

session="$(printf '%s' "$payload" | jq -r '.conversation_id // .session_id // empty' 2>/dev/null)"

kind=""
title=""
detail=""

case "$hook" in
  beforeSubmitPrompt)
    kind="task_start"
    title="New task"
    detail="$(printf '%s' "$payload" | jq -r '.prompt // .text // empty' 2>/dev/null | head -c 160)"
    ;;
  beforeShellExecution)
    kind="tool_use"
    title="Running a command"
    detail="$(printf '%s' "$payload" | jq -r '.command // empty' 2>/dev/null | head -c 120)"
    ;;
  afterFileEdit)
    kind="tool_use"
    title="Editing files"
    detail="$(printf '%s' "$payload" | jq -r '.file_path // .filePath // empty' 2>/dev/null)"
    ;;
  stop)
    kind="task_done"
    title="Finished"
    ;;
  *)
    exit 0
    ;;
esac

jq -n \
  --arg source "cursor" \
  --arg session "$session" \
  --arg kind "$kind" \
  --arg title "$title" \
  --arg detail "$detail" \
  '{source: $source, session: $session, kind: $kind, title: $title, detail: $detail}' \
  | curl -s -m 1 -X POST -H 'Content-Type: application/json' -d @- "$ENDPOINT" \
    >/dev/null 2>&1 || true

exit 0
