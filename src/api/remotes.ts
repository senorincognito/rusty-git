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

/** Points origin at a different URL (remote-tracking branches are kept). */
export const setOriginUrl = (path: string, url: string) =>
  invoke<void>("set_origin_remote_url", { path, url });

/** Commits that exist only on origin/<name> (not in HEAD or any local branch). */
export const countUnmergedRemoteCommits = (path: string, name: string) =>
  invoke<number>("count_unmerged_remote_commits", { path, name });
/** Deletes origin/<name> on the server. */
export const deleteRemoteBranch = (path: string, name: string) =>
  invoke<string>("delete_remote_branch", { path, name });

/** Renames origin/<name> on the server (push the new name, delete the old one). */
export const renameRemoteBranch = (path: string, name: string, newName: string) =>
  invoke<string>("rename_remote_branch", { path, name, newName });
