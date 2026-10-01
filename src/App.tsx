import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getRecentRepos,
  openRepo,
  pickFolder,
  removeRecentRepo,
  unwatchRepo,
  watchRepo,
  type RepoInfo,
} from "./git";
import BranchButton from "./BranchButton";
import Changes from "./Changes";
import Graph from "./Graph";
import Sidebar from "./Sidebar";
import SyncBar from "./SyncBar";
import TerminalPanel from "./TerminalPanel";
import "./App.css";

function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [recents, setRecents] = useState<RepoInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [graphKey, setGraphKey] = useState(0);
  const [terminalOpen, setTerminalOpen] = useState(false);

  const refreshRecents = useCallback(
    () => getRecentRepos().then(setRecents).catch(() => setRecents([])),
    [],
  );

  useEffect(() => {
    refreshRecents();
  }, [refreshRecents]);

  const open = useCallback(
    async (path: string) => {
      try {
        setError(null);
        setRepo(await openRepo(path));
        refreshRecents();
      } catch (e) {
        setError(String(e));
      }
    },
    [refreshRecents],
  );

  // Redraw the graph and refresh the branch label (a first commit creates the branch).
  const reload = useCallback((path: string) => {
    setGraphKey((k) => k + 1);
    openRepo(path).then(setRepo).catch(() => {});
  }, []);

  const onCommitted = useCallback(() => repo && reload(repo.path), [repo, reload]);

  // Follow changes made outside the app (terminal, editor, other tools) while a repo is open.
  const repoPath = repo?.path;
  useEffect(() => {
    if (!repoPath) return;
    watchRepo(repoPath).catch(() => {});
    const unlisten = listen("repo-changed", () => reload(repoPath));
    return () => {
      unlisten.then((fn) => fn());
      unwatchRepo().catch(() => {});
    };
  }, [repoPath, reload]);

  useEffect(() => {
    if (!repoPath) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.code === "Backquote") {
        e.preventDefault();
        setTerminalOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [repoPath]);

  const browse = useCallback(async () => {
    const path = await pickFolder();
    if (path) await open(path);
  }, [open]);

  if (repo) {
    return (
      <div className="shell">
        <header className="titlebar">
          <button className="ghost" onClick={() => setRepo(null)}>
            ← Repositories
          </button>
          <strong>{repo.name}</strong>
          <span className="branch">
            {repo.detached ? "detached @ " : ""}
            {repo.head ?? "(no commits yet)"}
          </span>
          <BranchButton path={repo.path} onCreated={onCommitted} />
          <SyncBar path={repo.path} refreshKey={graphKey} />
          <button
            className={"syncbtn termtoggle" + (terminalOpen ? " active" : "")}
            onClick={() => setTerminalOpen((o) => !o)}
            title="Toggle terminal (Ctrl+`)"
          >
            &gt;_ Terminal
          </button>
        </header>
        <div className="body">
          <Sidebar path={repo.path} refreshKey={graphKey} onChanged={onCommitted} />
          <Graph path={repo.path} refreshKey={graphKey} />
          <Changes path={repo.path} refreshKey={graphKey} onCommitted={onCommitted} />
        </div>
        <TerminalPanel path={repo.path} open={terminalOpen} onClose={() => setTerminalOpen(false)} />
      </div>
    );
  }

  return (
    <div className="welcome">
      <h1>Git Client</h1>
      <button className="primary" onClick={browse}>
        Open repository…
      </button>
      {error && <p className="error">{error}</p>}

      <h2>Recent</h2>
      {recents.length === 0 ? (
        <p className="muted">No recent repositories.</p>
      ) : (
        <ul className="recents">
          {recents.map((r) => (
            <li key={r.path}>
              <button className="recent" onClick={() => open(r.path)}>
                <span className="name">{r.name}</span>
                <span className="path">{r.path}</span>
              </button>
              <button
                className="ghost"
                title="Remove from list"
                onClick={() => removeRecentRepo(r.path).then(refreshRecents)}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export default App;
