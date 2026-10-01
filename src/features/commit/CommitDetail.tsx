import { useCallback, useEffect, useState } from "react";
import { getStatus } from "@/api/changes";
import { getCommitDetail, type CommitDetail as CommitDetailData } from "@/api/commit";
import FileBadge from "@/components/FileBadge";
import { useLatestRequest } from "@/hooks/useLatestRequest";

const dateFmt = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

/**
 * Right-hand panel for a selected commit: message, author and the files it changed.
 * While uncommitted changes exist, a notice at the top offers the way back to them.
 */
export default function CommitDetail({
  path,
  commit,
  refreshKey = 0,
  onClose,
}: {
  path: string;
  commit: { id: string; shortId: string };
  refreshKey?: number;
  onClose: () => void;
}) {
  const [detail, setDetail] = useState<CommitDetailData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [workingChanges, setWorkingChanges] = useState(0);
  const startDetail = useLatestRequest();
  const startStatus = useLatestRequest();

  useEffect(() => {
    const isCurrent = startDetail();
    setDetail(null);
    setError(null);
    getCommitDetail(path, commit.id)
      .then((d) => isCurrent() && setDetail(d))
      .catch((e) => isCurrent() && setError(String(e)));
  }, [path, commit.id, startDetail]);

  // How many files have uncommitted changes (a file that is both staged and edited counts once).
  const refreshStatus = useCallback(async () => {
    const isCurrent = startStatus();
    try {
      const changes = await getStatus(path);
      if (isCurrent()) setWorkingChanges(changes.length);
    } catch {
      if (isCurrent()) setWorkingChanges(0);
    }
  }, [path, startStatus]);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus, refreshKey]);

  useEffect(() => {
    window.addEventListener("focus", refreshStatus);
    return () => window.removeEventListener("focus", refreshStatus);
  }, [refreshStatus]);

  return (
    <aside className="commitdetail">
      {workingChanges > 0 && (
        <div className="wd-notice" role="status">
          <span>
            {workingChanges} file change{workingChanges === 1 ? "" : "s"} in working directory
          </span>
          <button className="secondary small" onClick={onClose}>
            View changes
          </button>
        </div>
      )}
      <header className="renamehead">
        <span>Commit</span>
        <code>{commit.shortId}</code>
        <button className="ghost" onClick={onClose} title="Close commit details">
          ×
        </button>
      </header>

      {error && <p className="error side-msg">{error}</p>}
      {!detail && !error && <p className="muted side-msg">Loading…</p>}

      {detail && (
        <>
          <div className="cd-meta">
            <p className="cd-summary">{detail.summary}</p>
            {detail.body && <pre className="cd-bodytext">{detail.body}</pre>}
            <p className="cd-line">
              {detail.author} · {dateFmt.format(new Date(detail.time * 1000))}
            </p>
            <p className="cd-line">
              {detail.parents.length === 0
                ? "Root commit"
                : `Parent${detail.parents.length === 1 ? "" : "s"}: ${detail.parents.join(", ")}`}
              {detail.isMerge && " · changes shown against the first parent"}
            </p>
          </div>

          <section className="filelist">
            <header>
              <span>
                Changed files <span className="count">{detail.totalFiles}</span>
              </span>
            </header>
            {detail.files.length === 0 && <p className="muted side-msg">This commit changes no files.</p>}
            <ul>
              {detail.files.map((f) => (
                <li key={f.path} title={f.oldPath ? `${f.oldPath} → ${f.path}` : f.path}>
                  <FileBadge kind={f.status} />
                  <span className="fname">{f.path}</span>
                </li>
              ))}
            </ul>
            {detail.truncated && (
              <p className="muted side-msg">
                Showing the first {detail.files.length} of {detail.totalFiles} files.
              </p>
            )}
          </section>
        </>
      )}
    </aside>
  );
}
