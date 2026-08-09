/**
 * deskmate adapter for opencode, written as an opencode plugin.
 *
 * Install: copy this file to `~/.config/opencode/plugin/deskmate.js`
 * (global) or `.opencode/plugin/deskmate.js` (per project).
 *
 * It maps opencode activity to deskmate events and POSTs them to the
 * local deskmate endpoint. All sends are fire-and-forget with a short
 * timeout, so a missing deskmate never slows opencode down.
 *
 * Event schema: docs/PROTOCOL.md in the deskmate repo.
 */

const PORT = process.env.DESKMATE_PORT || "8990";
const ENDPOINT = `http://127.0.0.1:${PORT}/event`;

function send(event) {
  try {
    fetch(ENDPOINT, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ source: "opencode", ...event }),
      signal: AbortSignal.timeout(1000),
    }).catch(() => {});
  } catch (_) {
    // fetch unavailable or aborted: stay silent, never break opencode.
  }
}

function clamp(text, n = 160) {
  if (typeof text !== "string") return "";
  return text.length > n ? text.slice(0, n - 1) + "…" : text;
}

export const DeskmatePlugin = async () => {
  return {
    // Fires before every tool execution.
    "tool.execute.before": async (input) => {
      const tool = input?.tool || "tool";
      let title = tool;
      let detail = "";
      const args = input?.args || {};
      if (tool === "bash" && args.command) {
        title = "Running a command";
        detail = clamp(args.command, 120);
      } else if (args.filePath || args.file_path) {
        title = `${tool}`;
        detail = clamp(String(args.filePath || args.file_path), 120);
      }
      send({ kind: "tool_use", session: input?.sessionID || "", title, detail });
    },

    // Bus events: session lifecycle, errors, etc.
    event: async ({ event }) => {
      if (!event || typeof event.type !== "string") return;
      const session = event.properties?.sessionID || "";

      switch (event.type) {
        case "session.idle":
          send({ kind: "task_done", session, title: "Finished" });
          break;
        case "session.error":
          send({
            kind: "error",
            session,
            title: "opencode error",
            detail: clamp(event.properties?.error?.data?.message || ""),
          });
          break;
        case "message.updated": {
          // A new user message means a task kicked off.
          const info = event.properties?.info;
          if (info?.role === "user") {
            send({ kind: "task_start", session, title: "New task" });
          }
          break;
        }
        case "permission.updated":
          send({
            kind: "notify",
            session,
            title: "Needs your approval",
            detail: clamp(event.properties?.title || ""),
          });
          break;
        default:
          break; // everything else is too chatty to bubble
      }
    },
  };
};
