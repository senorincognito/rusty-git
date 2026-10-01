import { useCallback, useEffect, useState } from "react";
import { pickFolder } from "@/api/dialog";
import { getRecentRepos, openRepo, removeRecentRepo, type RepoInfo } from "@/api/repo";

/** Start screen: open a repository from disk or from the recent list. */
export default function Welcome({ onOpen }: { onOpen: (repo: RepoInfo) => void }) {
  const [recents, setRecents] = useState<RepoInfo[]>([]);
  const [error, setError] = useState<string | null>(null);

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
        onOpen(await openRepo(path));
      } catch (e) {
        setError(String(e));
      }
    },
    [onOpen],
  );

  const browse = useCallback(async () => {
    const path = await pickFolder();
    if (path) await open(path);
  }, [open]);

  return (
    <div className="welcome">
      <h1>Rusty Git Client</h1>
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
