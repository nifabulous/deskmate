# opencode adapter

Feeds opencode activity to deskmate through the
[opencode plugin system](https://opencode.ai/docs/plugins/).

## Install

Copy the plugin where opencode loads plugins from:

```sh
# global (all projects)
mkdir -p ~/.config/opencode/plugin
cp deskmate.js ~/.config/opencode/plugin/

# or per project
mkdir -p .opencode/plugin
cp deskmate.js .opencode/plugin/
```

Start deskmate, then use opencode normally.

## What maps to what

| opencode signal          | deskmate event | Pet reaction             |
| ------------------------ | -------------- | ------------------------ |
| user message             | `task_start`   | Starts working animation |
| `tool.execute.before`    | `tool_use`     | Bubble with command/file |
| `session.idle`           | `task_done`    | Happy hop                |
| `session.error`          | `error`        | Shake + red bubble       |
| `permission.updated`     | `notify`       | Shake + bubble           |

Sends are fire-and-forget with a 1 second timeout, so opencode is never
blocked if deskmate isn't running.

If you changed the deskmate port, export `DESKMATE_PORT` before starting
opencode.

## Note on API drift

opencode's plugin API is young. The event names above match the plugin
docs at the time of writing; if a hook stops firing after an opencode
update, check their plugin docs and send a PR — this file is the whole
adapter.
