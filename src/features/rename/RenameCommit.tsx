import { useEffect, useRef, useState } from "react";
import { getRenameInfo, renameCommitMessage, type RenameInfo } from "@/api/history";
import "./RenameCommit.scss";

/** Right-hand panel for editing a commit's message. "Update" rewrites the commit. */
export default function RenameCommit({
  path,
  commit,
  onClose,
  onRenamed,
}: {
  path: string;
  commit: { id: string; shortId: string };
  onClose: () => void;
  onRenamed: () => void;
}) {
  const [info, setInfo] = useState<RenameInfo | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const box = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    let stale = false;
    setInfo(null);
    setError(null);
    getRenameInfo(path, commit.id)
      .then((i) => {
        if (stale) return;
        setInfo(i);
        setMessage(i.message);
      })
      .catch((e) => !stale && setError(String(e)));
    return () => {
      stale = true;
    };
  }, [path, commit.id]);

  useEffect(() => {
    if (info) {
      box.current?.focus();
      box.current?.select();
    }
  }, [info]);

  const changed = info !== null && message.trim() !== info.message.trim();
  const canUpdate = !busy && changed && message.trim().length > 0;

  const update = async () => {
    setBusy(true);
    try {
      await renameCommitMessage(path, commit.id, message);
      onRenamed();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside
      className="renamebox"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
        else if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && canUpdate) update();
      }}
    >
      <header className="panel-head">
        <span>Rename commit</span>
        <code>{commit.shortId}</code>
      </header>
      <div className="renamebody">
        <label htmlFor="rename-message">Commit message</label>
        <textarea
          id="rename-message"
          ref={box}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          disabled={!info || busy}
          placeholder={info || error ? undefined : "Loading…"}
        />
        {info?.pushed && (
          <p className="warn">
            This commit is already pushed. Renaming rewrites history, so it will need a force push.
          </p>
        )}
        {info && info.laterCommits > 0 && (
          <p className="note">
            {info.laterCommits} later commit{info.laterCommits === 1 ? "" : "s"} on this branch will be
            rewritten too (new ids, same content).
          </p>
        )}
        {error && <p className="error">{error}</p>}
      </div>
      <footer className="renamefoot">
        <button className="secondary" onClick={onClose} disabled={busy}>
          Cancel
        </button>
        <button className="primary" onClick={update} disabled={!canUpdate}>
          Update
        </button>
      </footer>
    </aside>
  );
}
