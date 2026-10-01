import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ChangeKind } from "@/api/changes";
import type { CommitFile } from "@/api/commit";
import { openRepo, type RepoInfo } from "@/api/repo";
import { unwatchRepo, watchRepo } from "@/api/watch";
import ResizablePanel from "@/components/ResizablePanel";
import Changes from "@/features/changes/Changes";
import CommitDetail from "@/features/commit/CommitDetail";
import FileDiff from "@/features/commit/FileDiff";
import Graph from "@/features/graph/Graph";
import RenameCommit from "@/features/rename/RenameCommit";
import Sidebar from "@/features/sidebar/Sidebar";
import TerminalPanel from "@/features/terminal/TerminalPanel";
import BranchButton from "@/features/toolbar/BranchButton";
import SyncBar from "@/features/toolbar/SyncBar";

/** The screen for an open repository: title bar, sidebar, graph, changes and terminal. */
export default function RepoView({
  repo,
  onRepoChange,
  onClose,
}: {
  repo: RepoInfo;
  onRepoChange: (repo: RepoInfo) => void;
  onClose: () => void;
}) {
  const [graphKey, setGraphKey] = useState(0);
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [renaming, setRenaming] = useState<{ id: string; shortId: string } | null>(null);
  // The commit whose files are shown in the right panel (instead of the working-directory changes).
  const [selectedCommit, setSelectedCommit] = useState<{ id: string; shortId: string } | null>(null);
  // A file of the selected commit shown in the centre instead of the graph.
  const [openFile, setOpenFile] = useState<CommitFile | null>(null);
  // An uncommitted file (staged or not) shown in the centre; chosen from the Changes panel.
  const [openWorkingFile, setOpenWorkingFile] = useState<{ path: string; staged: boolean; status: ChangeKind } | null>(null);
  const path = repo.path;

  const selectCommit = (commit: { id: string; shortId: string }) => {
    setSelectedCommit(commit);
    setOpenFile(null);
    setOpenWorkingFile(null);
  };
  const closeCommit = () => {
    setSelectedCommit(null);
    setOpenFile(null);
  };

  // Redraw everything and refresh the branch label (a first commit creates the branch).
  const reload = useCallback(() => {
    setGraphKey((k) => k + 1);
    openRepo(path).then(onRepoChange).catch(() => {});
  }, [path, onRepoChange]);

  // Follow changes made outside the app (terminal, editor, other tools).
  useEffect(() => {
    watchRepo(path).catch(() => {});
    const unlisten = listen("repo-changed", reload);
    return () => {
      unlisten.then((fn) => fn());
      unwatchRepo().catch(() => {});
    };
  }, [path, reload]);

  // Ctrl+` toggles the terminal.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.code === "Backquote") {
        e.preventDefault();
        setTerminalOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="shell">
      <header className="titlebar">
        <button className="ghost" onClick={onClose}>
          ← Repositories
        </button>
        <strong>{repo.name}</strong>
        <span className="branch">
          {repo.detached ? "detached @ " : ""}
          {repo.head ?? "(no commits yet)"}
        </span>
        <BranchButton path={path} onCreated={reload} />
        <SyncBar path={path} refreshKey={graphKey} />
        <button
          className={"syncbtn termtoggle" + (terminalOpen ? " active" : "")}
          onClick={() => setTerminalOpen((o) => !o)}
          title="Toggle terminal (Ctrl+`)"
        >
          &gt;_ Terminal
        </button>
      </header>
      <div className="body">
        <Sidebar
          path={path}
          refreshKey={graphKey}
          onChanged={reload}
          selectedId={selectedCommit?.id ?? null}
          onSelectCommit={selectCommit}
        />
        <div className="center">
          {/* The graph stays mounted (just hidden) while a file is open, so its scroll position survives. */}
          <div className={"center-pane" + (openFile || openWorkingFile ? " hidden" : "")}>
            <Graph
              path={path}
              refreshKey={graphKey}
              selectedId={selectedCommit?.id ?? null}
              onSelectCommit={selectCommit}
              onSelectWip={() => {
                closeCommit(); // back to the working-directory changes in the right panel
                setRenaming(null);
              }}
              onRenameCommit={setRenaming}
            />
          </div>
          {openFile && selectedCommit && (
            <FileDiff
              path={path}
              source={{ kind: "commit", id: selectedCommit.id, shortId: selectedCommit.shortId }}
              file={openFile}
              onClose={() => setOpenFile(null)}
            />
          )}
          {openWorkingFile && !openFile && (
            <FileDiff
              path={path}
              source={{ kind: openWorkingFile.staged ? "staged" : "unstaged" }}
              file={{ path: openWorkingFile.path, status: openWorkingFile.status }}
              refreshKey={graphKey}
              onClose={() => setOpenWorkingFile(null)}
            />
          )}
        </div>
        <ResizablePanel edge="left" storageKey="changesWidth" defaultWidth={340} min={260}>
          {renaming && (
            <RenameCommit
              path={path}
              commit={renaming}
              onClose={() => setRenaming(null)}
              onRenamed={() => {
                setRenaming(null);
                closeCommit(); // renaming gives the commit (and its successors) new ids
                reload();
              }}
            />
          )}
          {!renaming && selectedCommit && (
            <CommitDetail
              path={path}
              commit={selectedCommit}
              refreshKey={graphKey}
              selectedPath={openFile?.path ?? null}
              onSelectFile={setOpenFile}
              onClose={closeCommit}
            />
          )}
          <Changes
            path={path}
            refreshKey={graphKey}
            hidden={renaming !== null || selectedCommit !== null}
            selected={openWorkingFile && { path: openWorkingFile.path, staged: openWorkingFile.staged }}
            onSelectFile={setOpenWorkingFile}
            onCommitted={() => {
              setOpenWorkingFile(null); // what was committed no longer has a working diff
              reload();
            }}
          />
        </ResizablePanel>
      </div>
      <TerminalPanel path={path} open={terminalOpen} onClose={() => setTerminalOpen(false)} />
    </div>
  );
}
