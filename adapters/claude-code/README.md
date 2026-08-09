# Claude Code adapter

Feeds Claude Code activity to deskmate through [Claude Code hooks](https://docs.claude.com/en/docs/claude-code/hooks).

## Install

1. Copy the hook script somewhere stable and make it executable:

   ```sh
   mkdir -p ~/.deskmate
   cp deskmate-hook.sh ~/.deskmate/
   chmod +x ~/.deskmate/deskmate-hook.sh
   ```

2. Merge `settings.example.json` into your Claude Code settings:
   `~/.claude/settings.json` for all projects, or `.claude/settings.json`
   inside one project to scope it there.

3. Start deskmate, then run any Claude Code session. The pet starts
   working when you submit a prompt, bubbles the commands and file edits
   as they happen, and hops when the task finishes.

## What maps to what

| Claude Code hook   | deskmate event | Pet reaction              |
| ------------------ | -------------- | ------------------------- |
| `UserPromptSubmit` | `task_start`   | Starts working animation  |
| `PreToolUse`       | `tool_use`     | Bubble with command/file  |
| `Stop`             | `task_done`    | Happy hop                 |
| `Notification`     | `notify`       | Shake + bubble            |
| `SessionStart`     | `status`       | Wakes up                  |

The script requires `jq` and `curl`, exits silently if deskmate isn't
running, and never blocks Claude Code (1 second curl timeout, always
exits 0).

If you changed the deskmate port, export `DESKMATE_PORT` so the hook
script picks it up too.
