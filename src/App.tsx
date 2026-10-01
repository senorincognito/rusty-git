import { useState } from "react";
import type { RepoInfo } from "@/api/repo";
import RepoView from "@/features/repo/RepoView";
import Welcome from "@/features/welcome/Welcome";
import "./styles/main.scss";

function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);

  return repo ? (
    <RepoView repo={repo} onRepoChange={setRepo} onClose={() => setRepo(null)} />
  ) : (
    <Welcome onOpen={setRepo} />
  );
}

export default App;
