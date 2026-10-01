import { useCallback, useEffect, useRef, useState } from "react";
import { getFileDiff, getWorkingDiff, type FileDiff as FileDiffData } from "@/api/diff";
import FileBadge, { type FileStatus } from "@/components/FileBadge";
import { useLatestRequest } from "@/hooks/useLatestRequest";
import { usePersistentState } from "@/hooks/usePersistentState";

const ROW_H = 20;
const OVERSCAN = 20;

/** Where the diff comes from: a commit, or the staged / unstaged changes of the working tree. */
export type DiffSource = { kind: "commit"; id: string; shortId: string } | { kind: "staged" } | { kind: "unstaged" };

export interface DiffFile {
  path: string;
  oldPath?: string | null;
  status: FileStatus;
}

/**
 * The centre view for one file: its content with the added and removed lines marked in place
 * (or just the changed hunks). Rendered in a virtual list so long files stay fast.
 * Working-tree diffs reload when the repo changes or the window regains focus, keeping the scroll position.
 */
export default function FileDiff({
  path,
  source,
  file,
  refreshKey = 0,
  onClose,
}: {
  path: string;
  source: DiffSource;
  file: DiffFile;
  refreshKey?: number;
  onClose: () => void;
}) {
  const [full, setFull] = usePersistentState("diff.fullFile", true, (v): v is boolean => typeof v === "boolean");
  const [diff, setDiff] = useState<FileDiffData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewH, setViewH] = useState(600);
  const scroller = useRef<HTMLDivElement>(null);
  const start = useLatestRequest();

  const commitId = source.kind === "commit" ? source.id : null;
  const staged = source.kind === "staged";
  const isWorking = source.kind !== "commit";
  const oldPath = file.oldPath ?? null;

  const fetchDiff = useCallback(
    () =>
      commitId !== null
        ? getFileDiff(path, commitId, file.path, oldPath, full)
        : getWorkingDiff(path, file.path, staged, full),
    [path, commitId, staged, file.path, oldPath, full],
  );

  // A different file, source or view mode: start from the top.
  useEffect(() => {
    const isCurrent = start();
    setDiff(null);
    setError(null);
    setScrollTop(0);
    scroller.current?.scrollTo({ top: 0, left: 0 });
    fetchDiff()
      .then((d) => isCurrent() && setDiff(d))
      .catch((e) => isCurrent() && setError(String(e)));
  }, [fetchDiff, start]);

  // The working tree can change under an open diff (an edit, staging, a commit): refresh quietly.
  const firstRun = useRef(true);
  useEffect(() => {
    if (!isWorking) return;
    const refresh = () => {
      const isCurrent = start();
      fetchDiff()
        .then((d) => isCurrent() && setDiff(d))
        .catch(() => {});
    };
    if (firstRun.current) firstRun.current = false;
    else refresh();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [isWorking, refreshKey, fetchDiff, start]);

  // Escape closes the diff, like the back button. Leave it alone whenever Escape already means
  // something else: typing in a field (commit message, branch name, the terminal) or an open menu/dialog.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented || e.isComposing) return;
      const target = e.target as HTMLElement | null;
      if (target && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return;
      if (document.querySelector(".ctxmenu, .modal-backdrop")) return;
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setViewH(el.clientHeight));
    ro.observe(el);
    setViewH(el.clientHeight);
    return () => ro.disconnect();
  }, []);

  const lines = diff?.lines ?? [];
  const first = Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN);
  const last = Math.min(lines.length, Math.ceil((scrollTop + viewH) / ROW_H) + OVERSCAN);
  const maxNo = lines.reduce((m, l) => Math.max(m, l.oldNo ?? 0, l.newNo ?? 0), 0);
  const gutter = `${Math.max(String(maxNo).length, 2) + 1}ch`;
  const origin = source.kind === "commit" ? source.shortId : source.kind === "staged" ? "staged" : "unstaged";

  return (
    <section className="filediff">
      <header className="fd-head">
        <button
          className="ghost"
          onClick={onClose}
          title={isWorking ? "Close the diff (Esc)" : "Back to the commit graph (Esc)"}
        >
          {isWorking ? "← Back" : "← Back to graph"}
        </button>
        <FileBadge kind={file.status} />
        <span className="fd-path" title={file.path}>
          {file.path}
        </span>
        {file.oldPath && <span className="fd-from">renamed from {file.oldPath}</span>}
        <code className="fd-commit">{origin}</code>
        {diff && !diff.binary && (
          <span className="fd-stats">
            <span className="add">+{diff.additions}</span> <span className="del">-{diff.deletions}</span>
            {diff.truncated && <span className="muted"> · first {lines.length} lines shown</span>}
          </span>
        )}
        <label className="switch" title="Show the whole file, or only the changed parts with 3 lines of context">
          <input type="checkbox" role="switch" checked={full} onChange={(e) => setFull(e.target.checked)} />
          <span className="switch-track" aria-hidden="true" />
          <span>Full file</span>
        </label>
      </header>

      <div
        className="fd-body"
        ref={scroller}
        onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
        style={{ "--ln": gutter } as React.CSSProperties}
      >
        {error && <p className="error pad">{error}</p>}
        {!diff && !error && <p className="muted pad">Loading…</p>}
        {diff?.binary && <p className="muted pad">Binary or very large file: no preview.</p>}
        {diff && !diff.binary && lines.length === 0 && (
          <p className="muted pad">
            {isWorking
              ? `This file has no ${source.kind} changes (any more).`
              : "No content changes in this file (for example, only its mode changed)."}
          </p>
        )}
        {lines.length > 0 && (
          <div className="fd-list" style={{ height: lines.length * ROW_H }}>
            {lines.slice(first, last).map((l, i) => (
              <div key={first + i} className={`dl ${l.kind}`} style={{ top: (first + i) * ROW_H }}>
                {l.kind === "hunk" ? (
                  <span className="tx">{l.text}</span>
                ) : (
                  <>
                    <span className="ln">{l.oldNo ?? ""}</span>
                    <span className="ln">{l.newNo ?? ""}</span>
                    <span className="mk">{l.kind === "add" ? "+" : l.kind === "del" ? "-" : ""}</span>
                    <span className="tx">{l.text || " "}</span>
                  </>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}
