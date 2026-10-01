import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  createCommit,
  getStatus,
  stagePaths,
  unstagePaths,
  type ChangeKind,
  type FileChange,
} from "./git";

const BADGE: Record<ChangeKind, string> = {
  new: "A",
  modified: "M",
  deleted: "D",
  typechange: "T",
  conflicted: "!",
};

function FileList(props: {
  title: string;
  files: { path: string; kind: ChangeKind }[];
  actionLabel: string;
  onAction: (paths: string[]) => void;
  onActionAll: () => void;
}) {
  const { title, files, actionLabel, onAction, onActionAll } = props;
  return (
    <section className="filelist">
      <header>
        <span>
          {title} <span className="count">{files.length}</span>
        </span>
        {files.length > 0 && (
          <button className="ghost" onClick={onActionAll}>
            {actionLabel} all
          </button>
        )}
      </header>
      <ul>
        {files.map((f) => (
          <li key={f.path} title={f.path}>
            <span className={`badge ${f.kind}`}>{BADGE[f.kind]}</span>
            <span className="fname">{f.path}</span>
            <button className="ghost" onClick={() => onAction([f.path])}>
              {actionLabel}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export default function Changes({
  path,
  refreshKey = 0,
  onCommitted,
}: {
  path: string;
  refreshKey?: number;
  onCommitted: () => void;
}) {
  const [changes, setChanges] = useState<FileChange[]>([]);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Overlapping refreshes can finish out of order; only the latest request may update state.
  const latest = useRef(0);
  const refresh = useCallback(async () => {
    const id = ++latest.current;
    try {
      const c = await getStatus(path);
      if (id !== latest.current) return;
      setChanges(c);
      setError(null);
    } catch (e) {
      if (id === latest.current) setError(String(e));
    }
  }, [path]);

  useEffect(() => {
    setMessage("");
    refresh();
    // Pick up edits made outside the app when the window regains focus.
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [path, refresh]);

  // Reload when the parent reports a repo change (e.g. a commit or checkout made elsewhere).
  useEffect(() => {
    refresh();
  }, [refreshKey, refresh]);

  const staged = useMemo(
    () => changes.filter((c) => c.staged).map((c) => ({ path: c.path, kind: c.staged! })),
    [changes],
  );
  const unstaged = useMemo(
    () => changes.filter((c) => c.unstaged).map((c) => ({ path: c.path, kind: c.unstaged! })),
    [changes],
  );

  const run = async (op: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await op();
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      await refresh();
      setBusy(false);
    }
  };

  const commit = () =>
    run(async () => {
      await createCommit(path, message);
      setMessage("");
      onCommitted();
    });

  const canCommit = !busy && staged.length > 0 && message.trim().length > 0;

  return (
    <aside className="changes">
      <FileList
        title="Unstaged"
        files={unstaged}
        actionLabel="Stage"
        onAction={(p) => run(() => stagePaths(path, p))}
        onActionAll={() =>
          run(() =>
            stagePaths(
              path,
              unstaged.map((f) => f.path),
            ),
          )
        }
      />
      <FileList
        title="Staged"
        files={staged}
        actionLabel="Unstage"
        onAction={(p) => run(() => unstagePaths(path, p))}
        onActionAll={() =>
          run(() =>
            unstagePaths(
              path,
              staged.map((f) => f.path),
            ),
          )
        }
      />
      <div className="commitbox">
        <textarea
          placeholder="Commit message"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && canCommit) commit();
          }}
          rows={4}
        />
        {error && <p className="error">{error}</p>}
        <button className="primary" disabled={!canCommit} onClick={commit}>
          Commit{staged.length > 0 ? ` (${staged.length})` : ""}
        </button>
      </div>
    </aside>
  );
}
