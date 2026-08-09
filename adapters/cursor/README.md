# Cursor adapter (beta)

Feeds Cursor's agent activity to deskmate through
[Cursor agent hooks](https://cursor.com/docs/agent/hooks), which are
beta in Cursor. Expect this adapter to need small updates as Cursor's
hook payloads evolve; it reads every field defensively and always
answers `{"permission":"allow"}`, so worst case it does nothing rather
than getting in your way.

## Install

1. Copy the hook script somewhere stable and make it executable:

   ```sh
   mkdir -p ~/.deskmate
   cp deskmate-hook.sh ~/.deskmate/cursor-hook.sh
   chmod +x ~/.deskmate/cursor-hook.sh
   ```

2. Merge `hooks.example.json` into `~/.cursor/hooks.json` (create it if
   it doesn't exist), or into `.cursor/hooks.json` inside one project.

3. Restart Cursor, start deskmate, and run an agent task.

## What maps to what

| Cursor hook            | deskmate event | Pet reaction             |
| ---------------------- | -------------- | ------------------------ |
| `beforeSubmitPrompt`   | `task_start`   | Starts working animation |
| `beforeShellExecution` | `tool_use`     | Bubble with the command  |
| `afterFileEdit`        | `tool_use`     | Bubble with the file     |
| `stop`                 | `task_done`    | Happy hop                |

Requires `jq` and `curl`. The permission response is printed before the
deskmate call, and the curl has a 1 second timeout, so Cursor is never
blocked — with or without deskmate running.
