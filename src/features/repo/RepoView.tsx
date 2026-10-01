import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { openRepo, type RepoInfo } from "@/api/repo";
import { unwatchRepo, watchRepo } from "@/api/watch";
import Changes from "@/features/changes/Changes";
import Graph from "@/features/graph/Graph";
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
  const path = repo.path;

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
        <Sidebar path={path} refreshKey={graphKey} onChanged={reload} />
        <Graph path={path} refreshKey={graphKey} />
        <Changes path={path} refreshKey={graphKey} onCommitted={reload} />
      </div>
      <TerminalPanel path={path} open={terminalOpen} onClose={() => setTerminalOpen(false)} />
    </div>
  );
}
