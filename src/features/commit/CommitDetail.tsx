import { useCallback, useEffect, useState } from "react";
import { getStatus } from "@/api/changes";
import { getCommitDetail, type CommitDetail as CommitDetailData, type CommitFile } from "@/api/commit";
import { popStash } from "@/api/stash";
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
  selectedPath,
  onSelectFile,
  onStashPopped,
  onClose,
}: {
  path: string;
  commit: { id: string; shortId: string };
  refreshKey?: number;
  /** The file currently open in the centre view, if any. */
  selectedPath: string | null;
  onSelectFile: (file: CommitFile) => void;
  /** The stash shown here was applied and removed, so there is nothing left to show. */
  onStashPopped: () => void;
  onClose: () => void;
}) {
  const [detail, setDetail] = useState<CommitDetailData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [workingChanges, setWorkingChanges] = useState(0);
  const [popping, setPopping] = useState(false);
  const [popError, setPopError] = useState<string | null>(null);
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

  useEffect(() => {
    setPopError(null);
  }, [commit.id]);

  const pop = async () => {
    setPopping(true);
    setPopError(null);
    try {
      await popStash(path, commit.id);
      onStashPopped();
    } catch (e) {
      setPopError(String(e));
      setPopping(false);
    }
  };

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
            {detail.stash && (
              <>
                <p className="cd-line">
                  <strong>{detail.stash}</strong>: every uncommitted change that was saved here, including untracked files
                </p>
                <div className="stash-actions">
                  <button
                    className="primary small"
                    disabled={popping || workingChanges > 0}
                    onClick={pop}
                    title={
                      workingChanges > 0
                        ? "Commit or stash your uncommitted changes first"
                        : "Apply this stash to the working directory and remove it from the list"
                    }
                  >
                    {popping ? "Popping…" : "Pop"}
                  </button>
                  <span className="muted">
                    {workingChanges > 0
                      ? "Needs a clean working directory"
                      : "Applies it and removes it from the list"}
                  </span>
                </div>
                {popError && <p className="error cd-line">{popError}</p>}
              </>
            )}
            <p className="cd-line">
              {detail.stash
                ? `Made on ${detail.parents[0] ?? "an unknown commit"}`
                : detail.parents.length === 0
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
                <li
                  key={f.path}
                  className={"selectable" + (f.path === selectedPath ? " selected" : "")}
                  title={f.oldPath ? `${f.oldPath} → ${f.path}` : f.path}
                  role="button"
                  tabIndex={0}
                  onClick={() => onSelectFile(f)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onSelectFile(f);
                    }
                  }}
                >
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
