// The harness layer of the privacy guard: before a write or an edit lands, the
// payload is handed to scripts/hooks/privacy-hook.sh, which runs the one guard
// (scripts/privacy/guard.py). A refusal names the rule and the file, never the
// matched text. The payload is handed as spawnSync's
// `input` so stdin closes with it: the guard reads stdin to EOF (json.load),
// and a payload merely written to an open pipe leaves that guard waiting
// forever -- the stall a session with the old plugin measured. Read and bash are not scanned here; the git hooks and
// the gate remain the backstop for anything written by other means.
// The V2 loader rejects a named-export module outright ("Plugin must export a
// default definition with an id and an effect or setup function", seen as a
// WARN in ~/.local/share/opencode/log), so the definition is the default
// export and the hook registers on the tool domain's execute.before with
// event.input -- the V1 (input, output) map-of-events shape never loaded.
import { spawnSync } from "node:child_process";

export default {
  id: "privacy-guard",
  setup: async (ctx) => {
    const root = ctx.location.project?.canonical || ctx.location.directory;
    if (!root) return;
    const dir = ctx.location.directory || root;
    const launcher = `${root}/scripts/hooks/privacy-hook.sh`;
    const registration = await ctx.tool.hook("execute.before", (event) =>
      guardWrite(event, launcher, dir, root));
    return () => {
      registration.dispose();
    };
  },
};

// The guard launcher gets the payload as stdin and sees it through to EOF
// (input ends stdin), with a bounded wait so a hung guard is reported as
// not-run rather than stalling the write it cannot answer for.
function runGuard(launcher, payload, root) {
  const res = spawnSync("bash", [launcher], {
    cwd: root,
    input: `${payload}\n`,
    timeout: 15000,
    killSignal: "SIGKILL",
  });
  if (res.error) return [3, String(res.error.message ?? res.error)];
  return [res.status ?? res.code ?? 3, String(res.stderr ?? "")];
}

// execute.before fires for every tool; only writes and edits carry a path and
// new text. A relative tool path is relative to the session's directory, not
// to the process running the guard, so it is absolutized before handoff -- a
// relative path left to resolve in the launcher's cwd maps outside the repo
// and the guard answers 0 without ever reading the text.
function guardWrite(event, launcher, dir, root) {
  const args = event.input || {};
  const path = args.path || args.filePath || args.notebook_path;
  if (!path) return;
  const absolute = /^(?:\/|[A-Za-z]:[\\/])/.test(path) ? path : `${dir}/${path}`;
  const changed =
    typeof args.content === "string" ||
    typeof args.newString === "string" ||
    Array.isArray(args.edits);
  if (!changed) return;

  const toolInput = { file_path: absolute };
  if (typeof args.content === "string") toolInput.content = args.content;
  if (typeof args.newString === "string") toolInput.new_string = args.newString;
  if (typeof args.oldString === "string") toolInput.old_string = args.oldString;
  if (Array.isArray(args.edits)) {
    toolInput.edits = args.edits.map((edit) => ({
      old_string: edit.oldString || "",
      new_string: edit.newString || "",
    }));
  }

  const payload = JSON.stringify({ tool_input: toolInput });
  const [code, reason] = runGuard(launcher, payload, root);
  if (code === 2) throw new Error(reason.trim());
  if (code !== 0) console.error(`privacy guard did not run: ${reason.trim()}`);
}
