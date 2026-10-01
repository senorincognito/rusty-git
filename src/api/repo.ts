import { invoke } from "@tauri-apps/api/core";

export interface RepoInfo {
  path: string;
  name: string;
  head: string | null;
  detached: boolean;
}

export const openRepo = (path: string) => invoke<RepoInfo>("open_repo", { path });
export const getRecentRepos = () => invoke<RepoInfo[]>("get_recent_repos");
export const removeRecentRepo = (path: string) =>
  invoke<void>("remove_recent_repo", { path });
