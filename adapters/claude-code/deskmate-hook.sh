#!/usr/bin/env bash
# deskmate adapter for Claude Code hooks.
#
# Claude Code pipes a JSON payload on stdin for every hook event. This script
# maps it to a deskmate event and POSTs it to the local deskmate endpoint.
# It always exits 0 and never blocks, so a missing or stopped deskmate can
# never interfere with Claude Code itself.
#
# Install: see adapters/claude-code/README.md

set -u
PORT="${DESKMATE_PORT:-8990}"
ENDPOINT="http://127.0.0.1:${PORT}/event"

payload="$(cat 2>/dev/null || true)"
[ -z "$payload" ] && exit 0

command -v jq >/dev/null 2>&1 || exit 0
command -v curl >/dev/null 2>&1 || exit 0

hook="$(printf '%s' "$payload" | jq -r '.hook_event_name // empty')"
session="$(printf '%s' "$payload" | jq -r '.session_id // empty')"

kind=""
title=""
detail=""

case "$hook" in
  UserPromptSubmit)
    kind="task_start"
    title="New task"
    detail="$(printf '%s' "$payload" | jq -r '.prompt // empty' | head -c 160)"
    ;;
  PreToolUse)
    kind="tool_use"
    tool="$(printf '%s' "$payload" | jq -r '.tool_name // empty')"
    case "$tool" in
      Bash)      title="Running a command"
                 detail="$(printf '%s' "$payload" | jq -r '.tool_input.command // empty' | head -c 120)" ;;
      Edit|Write) title="Editing files"
                 detail="$(printf '%s' "$payload" | jq -r '.tool_input.file_path // empty')" ;;
      Read)      kind="tool_use"; title="" ;;   # too chatty to bubble
      *)         title="$tool" ;;
    esac
    ;;
  Stop)
    kind="task_done"
    title="Finished"
    ;;
  Notification)
    kind="notify"
    title="Claude Code"
    detail="$(printf '%s' "$payload" | jq -r '.message // empty' | head -c 160)"
    ;;
  SessionStart)
    kind="status"
    title="Session started"
    ;;
  *)
    exit 0
    ;;
esac

jq -n \
  --arg source "claude-code" \
  --arg session "$session" \
  --arg kind "$kind" \
  --arg title "$title" \
  --arg detail "$detail" \
  '{source: $source, session: $session, kind: $kind, title: $title, detail: $detail}' \
  | curl -s -m 1 -X POST -H 'Content-Type: application/json' -d @- "$ENDPOINT" \
    >/dev/null 2>&1 || true

exit 0
