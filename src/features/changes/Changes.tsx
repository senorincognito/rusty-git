import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  createCommit,
  getHeadCommit,
  discardPaths,
  getStatus,
  stagePaths,
  unstagePaths,
  type ChangeKind,
  type FileChange,
  type HeadCommit,
} from "@/api/changes";
import { confirmDialog } from "@/api/dialog";
import { stashPaths } from "@/api/stash";
import ContextMenu, { type MenuItem } from "@/components/ContextMenu";
import FileBadge from "@/components/FileBadge";
import { useLatestRequest } from "@/hooks/useLatestRequest";
import StashDialog from "./StashDialog";
import "./Changes.scss";

function FileList(props: {
  title: string;
  files: { path: string; kind: ChangeKind }[];
  actionLabel: string;
  onAction: (paths: string[]) => void;
  onActionAll: () => void;
  /** The file whose diff is open in the centre, if it is in this list. */
  selectedPath: string | null;
  onSelect: (file: { path: string; kind: ChangeKind }) => void;
  /** Right-click on a row. */
  onContextMenu: (file: { path: string; kind: ChangeKind }, x: number, y: number) => void;
  /** The file whose context menu is open, if it is in this list. */
  menuPath: string | null;
}) {
  const { title, files, actionLabel, onAction, onActionAll, selectedPath, onSelect, onContextMenu, menuPath } = props;
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
          <li
            key={f.path}
            className={"selectable" + (f.path === selectedPath ? " selected" : "") + (f.path === menuPath ? " ctx" : "")}
            title={f.path}
            role="button"
            tabIndex={0}
            onClick={() => onSelect(f)}
            onContextMenu={(e) => {
              e.preventDefault();
              onContextMenu(f, e.clientX, e.clientY);
            }}
            onKeyDown={(e) => {
              // Only for the row itself, not for keys pressed on the Stage/Unstage button inside it.
              if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) {
                e.preventDefault();
                onSelect(f);
              }
            }}
          >
            <FileBadge kind={f.kind} />
            <span className="fname">{f.path}</span>
            <button
              className="ghost"
              onClick={(e) => {
                e.stopPropagation(); // staging a file must not also open its diff
                onAction([f.path]);
              }}
            >
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
  hidden = false,
  selected = null,
  onSelectFile,
  onCommitted,
}: {
  path: string;
  refreshKey?: number;
  /** Keep mounted (so the draft message survives) but not visible. */
  hidden?: boolean;
  /** The file whose diff is open in the centre (a file can be listed both staged and unstaged). */
  selected?: { path: string; staged: boolean } | null;
  onSelectFile: (file: { path: string; staged: boolean; status: ChangeKind }) => void;
  onCommitted: () => void;
}) {
  const [changes, setChanges] = useState<FileChange[]>([]);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [amend, setAmend] = useState(false);
  const [stashOpen, setStashOpen] = useState(false);
  const [head, setHead] = useState<HeadCommit | null>(null);
  // Right-click menu on a file row (which list it was opened in decides what it offers).
  const [menu, setMenu] = useState<{ x: number; y: number; path: string; kind: ChangeKind; staged: boolean } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const draft = useRef(""); // the message typed before switching amend on

  const start = useLatestRequest();
  const refresh = useCallback(async () => {
    const isCurrent = start();
    try {
      const c = await getStatus(path);
      if (!isCurrent()) return;
      setChanges(c);
      setError(null);
    } catch (e) {
      if (isCurrent()) setError(String(e));
    }
  }, [path, start]);

  useEffect(() => {
    setMessage("");
    setAmend(false);
    draft.current = "";
    refresh();
    // Pick up edits made outside the app when the window regains focus.
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [path, refresh]);

  // Reload when the parent reports a repo change (e.g. a commit or checkout made elsewhere).
  useEffect(() => {
    refresh();
  }, [refreshKey, refresh]);

  // The last commit: used to pre-fill the amend message and to warn about rewriting pushed history.
  const startHead = useLatestRequest();
  useEffect(() => {
    const isCurrent = startHead();
    getHeadCommit(path)
      .then((h) => isCurrent() && setHead(h))
      .catch(() => isCurrent() && setHead(null));
  }, [path, refreshKey, startHead]);

  // The last commit disappeared (e.g. reset in a terminal): there is nothing left to amend.
  useEffect(() => {
    if (amend && !head) setAmend(false);
  }, [amend, head]);

  const toggleAmend = (on: boolean) => {
    if (on) {
      draft.current = message;
      setMessage(head?.message ?? "");
    } else {
      setMessage(draft.current);
    }
    setAmend(on);
  };

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

  // Discard throws the edits away for good: ask first, and say what happens to the file.
  const discard = async (file: { path: string; kind: ChangeKind }) => {
    const text =
      file.kind === "new"
        ? `Delete the untracked file "${file.path}"? It is not in git, so it cannot be recovered.`
        : file.kind === "deleted"
          ? `Restore "${file.path}", which you deleted?`
          : `Discard your changes to "${file.path}"? They are lost for good (git cannot recover them).${
              staged.some((f) => f.path === file.path) ? " Changes that are staged stay staged." : ""
            }`;
    if (!(await confirmDialog(text, "Discard changes", true, file.kind === "deleted" ? "Restore" : "Discard"))) return;
    await run(async () => {
      await discardPaths(path, [file.path]);
      onCommitted(); // reloads everything and closes a diff that is no longer true
    });
  };

  const stashFile = (file: { path: string }) =>
    run(async () => {
      await stashPaths(path, [file.path]);
      onCommitted();
    });

  const menuItems = (m: NonNullable<typeof menu>): MenuItem[] => {
    const file = { path: m.path, kind: m.kind };
    const stashItem: MenuItem = {
      label: "Stash",
      disabled: busy,
      title: "Move this file's uncommitted changes (staged and unstaged) into a new stash; everything else stays",
      onClick: () => stashFile(file),
    };
    if (m.staged) {
      return [{ label: "Unstage", disabled: busy, onClick: () => run(() => unstagePaths(path, [m.path])) }, stashItem];
    }
    return [
      { label: "Stage", disabled: busy, onClick: () => run(() => stagePaths(path, [m.path])) },
      {
        label: m.kind === "new" ? "Delete file…" : m.kind === "deleted" ? "Restore file…" : "Discard changes…",
        danger: true,
        disabled: busy || m.kind === "conflicted",
        title:
          m.kind === "conflicted"
            ? "This file has merge conflicts"
            : "Throw away your unstaged changes to this file (asks first)",
        onClick: () => discard(file),
      },
      stashItem,
    ];
  };

  const commit = () =>
    run(async () => {
      await createCommit(path, message, amend);
      setMessage("");
      setAmend(false);
      draft.current = "";
      onCommitted();
    });

  // An amend may change only the message, so it doesn't need staged files.
  const canCommit = !busy && message.trim().length > 0 && (amend || staged.length > 0);

  return (
    <aside className="changes" style={hidden ? { display: "none" } : undefined}>
      <FileList
        title="Unstaged"
        files={unstaged}
        selectedPath={selected && !selected.staged ? selected.path : null}
        onSelect={(f) => onSelectFile({ path: f.path, staged: false, status: f.kind })}
        menuPath={menu && !menu.staged ? menu.path : null}
        onContextMenu={(f, x, y) => setMenu({ x, y, path: f.path, kind: f.kind, staged: false })}
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
        selectedPath={selected?.staged ? selected.path : null}
        onSelect={(f) => onSelectFile({ path: f.path, staged: true, status: f.kind })}
        menuPath={menu?.staged ? menu.path : null}
        onContextMenu={(f, x, y) => setMenu({ x, y, path: f.path, kind: f.kind, staged: true })}
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
        <div className="commit-toolbar">
          <label
            className={"switch" + (!head || busy ? " disabled" : "")}
            title={
              head
                ? `Replace the last commit (${head.shortId}) instead of creating a new one`
                : "There is no commit to amend yet"
            }
          >
            <input
              type="checkbox"
              role="switch"
              checked={amend}
              disabled={!head || busy}
              onChange={(e) => toggleAmend(e.target.checked)}
            />
            <span className="switch-track" aria-hidden="true" />
            <span>Amend previous commit</span>
          </label>
        </div>
        {amend && head?.pushed && (
          <p className="warn">
            This commit is already pushed. Amending it rewrites history, so it will need a force push.
          </p>
        )}
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
        <div className="commit-actions">
          <button
            className="secondary"
            disabled={busy || changes.length === 0}
            onClick={() => setStashOpen(true)}
            title="Move all uncommitted changes, including untracked files, into a stash"
          >
            Stash…
          </button>
          <button className="primary" disabled={!canCommit} onClick={commit}>
            {amend ? "Amend commit" : "Commit"}
            {staged.length > 0 ? ` (${staged.length})` : ""}
          </button>
        </div>
      </div>
      {menu && <ContextMenu x={menu.x} y={menu.y} onClose={closeMenu} items={menuItems(menu)} />}
      {stashOpen && (
        <StashDialog
          path={path}
          fileCount={changes.length}
          onClose={() => setStashOpen(false)}
          onStashed={() => {
            setStashOpen(false);
            onCommitted(); // reloads everything; also closes an open working-tree diff
          }}
        />
      )}
    </aside>
  );
}
