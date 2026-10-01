import { useCallback, useEffect, useRef, useState } from "react";
import { getSyncStatus, gitFetch, gitPull, gitPush, type SyncStatus } from "./git";

type Op = "fetch" | "pull" | "push";
type Notice = { kind: "ok" | "error"; text: string };

const OPS: Record<Op, { label: string; run: (path: string) => Promise<string> }> = {
  fetch: { label: "Fetch", run: gitFetch },
  pull: { label: "Pull", run: gitPull },
  push: { label: "Push", run: gitPush },
};

export default function SyncBar({ path, refreshKey }: { path: string; refreshKey: number }) {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [busy, setBusy] = useState<Op | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const latest = useRef(0);

  const refresh = useCallback(async () => {
    const id = ++latest.current;
    try {
      const s = await getSyncStatus(path);
      if (id === latest.current) setStatus(s);
    } catch {
      if (id === latest.current) setStatus(null);
    }
  }, [path]);

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

  const noRemote = status ? !status.hasRemote : false;
  const noBranch = status ? status.branch === null : true;
  const btn = (op: Op, disabled: boolean, title: string, badge?: string) => (
    <button className="syncbtn" disabled={busy !== null || disabled} title={title} onClick={() => run(op)}>
      {busy === op ? `${OPS[op].label}…` : OPS[op].label}
      {badge && busy !== op && <span className="badge-count">{badge}</span>}
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
        {btn(
          "push",
          noRemote || noBranch,
          status?.upstream ? `Push to ${status.upstream}` : "Publish this branch",
          status?.upstream ? (status.ahead ? `↑${status.ahead}` : undefined) : "new",
        )}
      </div>
      {notice && (
        <div className={`notice ${notice.kind}`} onClick={() => setNotice(null)} title="Click to dismiss">
          <pre>{notice.text}</pre>
        </div>
      )}
    </>
  );
}
