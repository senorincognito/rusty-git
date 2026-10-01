import { useCallback, useEffect, useState } from "react";
import { confirmDialog } from "@/api/dialog";
import { getSyncStatus, gitFetch, gitForcePush, gitPull, gitPush, type SyncStatus } from "@/api/sync";
import ContextMenu from "@/components/ContextMenu";
import { useLatestRequest } from "@/hooks/useLatestRequest";

type Op = "fetch" | "pull" | "push" | "force";
type Notice = { kind: "ok" | "error"; text: string };

const OPS: Record<Op, { label: string; run: (path: string) => Promise<string> }> = {
  fetch: { label: "Fetch", run: gitFetch },
  pull: { label: "Pull", run: gitPull },
  push: { label: "Push", run: gitPush },
  force: { label: "Force push", run: gitForcePush },
};

export default function SyncBar({ path, refreshKey }: { path: string; refreshKey: number }) {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [busy, setBusy] = useState<Op | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const start = useLatestRequest();

  const refresh = useCallback(async () => {
    const isCurrent = start();
    try {
      const s = await getSyncStatus(path);
      if (isCurrent()) setStatus(s);
    } catch {
      if (isCurrent()) setStatus(null);
    }
  }, [path, start]);

  // Reload when the repo changes (a fetch updates remote refs, which the watcher reports).
  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  useEffect(() => {
    setNotice(null);
  }, [path]);

  // Successful results fade away; errors stay until dismissed.
  useEffect(() => {
    if (notice?.kind !== "ok") return;
    const t = setTimeout(() => setNotice(null), 5000);
    return () => clearTimeout(t);
  }, [notice]);

  const run = async (op: Op) => {
    setBusy(op);
    setNotice(null);
    try {
      const out = await OPS[op].run(path);
      setNotice({ kind: "ok", text: out || `${OPS[op].label} complete` });
    } catch (e) {
      setNotice({ kind: "error", text: String(e) });
    } finally {
      setBusy(null);
      refresh();
    }
  };

  const forcePush = async () => {
    if (!status?.upstream) return;
    const behind = status.behind;
    const message = [
      `Force push "${status.branch}" to ${status.upstream}?`,
      "",
      "This overwrites the remote branch with your local history.",
      behind > 0
        ? `${behind} commit${behind === 1 ? "" : "s"} on ${status.upstream} that are not in your branch will be discarded there.`
        : "",
      "",
      "The push is refused if the remote changed since your last fetch.",
    ]
      .filter((line, i, all) => line !== "" || (i > 0 && all[i - 1] !== ""))
      .join("\n");
    if (await confirmDialog(message, "Force push", true, "Force push")) await run("force");
  };

  const noRemote = status ? !status.hasRemote : false;
  const noBranch = status ? status.branch === null : true;
  // A force push in progress is shown on the Push button it came from.
  const shown = (op: Op) => busy === op || (op === "push" && busy === "force");
  const btn = (op: Op, disabled: boolean, title: string, badge?: string) => (
    <button className="syncbtn" disabled={busy !== null || disabled} title={title} onClick={() => run(op)}>
      {shown(op) ? `${OPS[op].label}…` : OPS[op].label}
      {badge && !shown(op) && <span className="badge-count">{badge}</span>}
    </button>
  );

  return (
    <>
      <div className="syncbar">
        {btn("fetch", noRemote, noRemote ? "No remotes configured" : "Fetch all remotes")}
        {btn(
          "pull",
          noRemote || noBranch || !status?.upstream,
          status?.upstream ? `Pull (fast-forward) from ${status.upstream}` : "No upstream branch",
          status?.behind ? `↓${status.behind}` : undefined,
        )}
        <div
          className="splitbtn"
          onContextMenu={(e) => {
            e.preventDefault();
            setMenu({ x: e.clientX, y: e.clientY });
          }}
        >
          {btn(
            "push",
            noRemote || noBranch,
            status?.upstream ? `Push to ${status.upstream}` : "Publish this branch",
            status?.upstream ? (status.ahead ? `↑${status.ahead}` : undefined) : "new",
          )}
          {/* stopPropagation keeps the menu's outside-click handler from closing it right before this toggles it */}
          <button
            className="syncbtn splitarrow"
            aria-haspopup="menu"
            aria-expanded={menu !== null}
            title="More push options"
            disabled={busy !== null}
            onMouseDown={(e) => e.stopPropagation()}
            onClick={(e) => {
              if (menu) return closeMenu();
              const r = e.currentTarget.getBoundingClientRect();
              setMenu({ x: r.left, y: r.bottom + 4 });
            }}
          >
            ▾
          </button>
        </div>
      </div>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={closeMenu}
          items={[
            {
              label: "Force push",
              danger: true,
              disabled: busy !== null || noRemote || noBranch || !status?.upstream,
              title: noRemote
                ? "No remotes configured"
                : noBranch
                  ? "Check out a branch first"
                  : !status?.upstream
                    ? "This branch has not been pushed yet. Use Push first."
                    : `Overwrite ${status.upstream} with your branch (asks first)`,
              onClick: forcePush,
            },
          ]}
        />
      )}
      {notice && (
        <div className={`notice ${notice.kind}`} onClick={() => setNotice(null)} title="Click to dismiss">
          <pre>{notice.text}</pre>
        </div>
      )}
    </>
  );
}
