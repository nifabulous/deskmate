# deskmate event protocol (v1)

deskmate listens on a local HTTP endpoint. Any tool that can make an HTTP
request can drive the pet — no SDK required.

## Endpoint

```
POST http://127.0.0.1:8990/event
Content-Type: application/json
```

The port defaults to `8990` and can be changed with the `DESKMATE_PORT`
environment variable (set it for both deskmate and your adapter).

`GET /health` returns `{"ok":true,"app":"deskmate"}` and is the cheap way
for an adapter to check whether deskmate is running.

## Event shape

```json
{
  "source": "claude-code",
  "session": "abc123",
  "kind": "task_start",
  "title": "Review PR #104",
  "detail": "Reading the diff and checking tests"
}
```

| Field     | Type   | Required | Notes                                          |
| --------- | ------ | -------- | ---------------------------------------------- |
| `kind`    | string | yes      | One of the kinds below. Unknown kinds are treated as `status`. |
| `source`  | string | no       | Adapter name, e.g. `claude-code`, `opencode`.  |
| `session` | string | no       | Opaque session/task id from the source tool.   |
| `title`   | string | no       | Short line shown bold in the bubble.           |
| `detail`  | string | no       | Longer text, clamped to two lines in the UI.   |

Extra fields are accepted and ignored, so adapters can carry their own
metadata without breaking older deskmate versions.

## Kinds

| Kind         | Meaning                        | Pet reaction               |
| ------------ | ------------------------------ | -------------------------- |
| `task_start` | The agent began a task         | Working animation + bubble |
| `tool_use`   | The agent used a tool          | Working; bubble if titled  |
| `task_done`  | The task finished successfully | Happy hop + bubble         |
| `error`      | Something failed               | Shake + red bubble         |
| `notify`     | The agent needs attention      | Shake + bubble             |
| `status`     | Anything else worth showing    | Bubble only                |

With no events for 5 minutes the pet falls asleep; any event wakes it.

## Try it from a shell

```sh
curl -X POST http://127.0.0.1:8990/event \
  -H 'Content-Type: application/json' \
  -d '{"kind":"task_start","title":"Hello deskmate","detail":"Sent from curl"}'
```

## Writing an adapter

An adapter is anything that translates a tool's activity into these
events. The Claude Code adapter (`adapters/claude-code/`) is ~80 lines of
shell and is the reference implementation. Guidelines:

1. Never block the host tool. Use short timeouts and always exit 0.
2. Fail silent when deskmate isn't running.
3. Keep `title` short; put the rest in `detail`.
4. Don't send secrets. Bubble text is drawn on screen.

Planned adapters (contributions welcome): opencode (plugin API), Cursor
(agent hooks), and a `deskmate notify` CLI for scripts and CI.
