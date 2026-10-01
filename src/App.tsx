import { useCallback, useEffect, useState } from "react";
import {
  getRecentRepos,
  openRepo,
  pickFolder,
  removeRecentRepo,
  type RepoInfo,
} from "./git";
import Changes from "./Changes";
import Graph from "./Graph";
import "./App.css";

function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [recents, setRecents] = useState<RepoInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [graphKey, setGraphKey] = useState(0);

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

  // After a commit: redraw the graph and refresh the branch label (first commit creates the branch).
  const onCommitted = useCallback(() => {
    setGraphKey((k) => k + 1);
    if (repo) openRepo(repo.path).then(setRepo).catch(() => {});
  }, [repo]);

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
        </header>
        <div className="body">
          <Graph path={repo.path} refreshKey={graphKey} />
          <Changes path={repo.path} onCommitted={onCommitted} />
        </div>
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
