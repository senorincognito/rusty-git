import { useEffect, useState } from "react";
import type { RepoInfo } from "@/api/repo";
import CredentialPrompt from "@/features/auth/CredentialPrompt";
import RepoView from "@/features/repo/RepoView";
import Welcome from "@/features/welcome/Welcome";
import "./styles/main.scss";

function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);

  // The webview's own right-click menu (Back, Reload, Inspect, ...) has no place in the app: only the menus the app
  // defines open. Components that handle a right-click call preventDefault themselves (React runs before this
  // document listener), so what is left is everything else. Text fields keep the native cut/copy/paste menu.
  useEffect(() => {
    const onContextMenu = (e: MouseEvent) => {
      const target = e.target as HTMLElement | null;
      if (e.defaultPrevented || target?.closest("input, textarea, [contenteditable=true]")) return;
      e.preventDefault();
    };
    document.addEventListener("contextmenu", onContextMenu);
    return () => document.removeEventListener("contextmenu", onContextMenu);
  }, []);

  return (
    <>
      {repo ? (
        <RepoView repo={repo} onRepoChange={setRepo} onClose={() => setRepo(null)} />
      ) : (
        <Welcome onOpen={setRepo} />
      )}
      <CredentialPrompt />
    </>
  );
}

export default App;
