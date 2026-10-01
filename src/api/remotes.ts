import { invoke } from "@tauri-apps/api/core";

export interface RemoteInfo {
  name: string;
  url: string;
  branches: string[];
  /** Remote branch tracked by the checked-out local branch, if any. */
  trackedByHead: string | null;
}

/** The origin remote, or null if the repo has none. */
export const getOrigin = (path: string) => invoke<RemoteInfo | null>("get_origin", { path });
export const addOriginRemote = (path: string, url: string) =>
  invoke<void>("add_origin_remote", { path, url });

/** Commits that exist only on origin/<name> (not in HEAD or any local branch). */
export const countUnmergedRemoteCommits = (path: string, name: string) =>
  invoke<number>("count_unmerged_remote_commits", { path, name });
/** Deletes origin/<name> on the server. */
export const deleteRemoteBranch = (path: string, name: string) =>
  invoke<string>("delete_remote_branch", { path, name });
