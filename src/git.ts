import { invoke } from "@tauri-apps/api/core";
import { ask, open } from "@tauri-apps/plugin-dialog";

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

/** Shows the native folder picker; resolves to null if cancelled. */
export async function pickFolder(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title: "Open repository" });
  return typeof selected === "string" ? selected : null;
}

export interface RefLabel {
  name: string;
  kind: "branch" | "remote" | "tag";
  isHead: boolean;
}

export interface Edge {
  col: number;
  color: number;
}

export interface GraphRow {
  id: string;
  shortId: string;
  summary: string;
  author: string;
  email: string;
  /** Unix seconds. */
  time: number;
  parents: string[];
  refs: RefLabel[];
  col: number;
  color: number;
  top: Edge[];
  through: Edge[];
  bottom: Edge[];
}

export interface Graph {
  rows: GraphRow[];
  maxLanes: number;
  hasMore: boolean;
}

export const getGraph = (path: string, limit: number) =>
  invoke<Graph>("get_graph", { path, limit });

export type ChangeKind = "new" | "modified" | "deleted" | "typechange" | "conflicted";

export interface FileChange {
  path: string;
  staged: ChangeKind | null;
  unstaged: ChangeKind | null;
}

export const getStatus = (path: string) => invoke<FileChange[]>("get_status", { path });
export const stagePaths = (path: string, paths: string[]) =>
  invoke<void>("stage_paths", { path, paths });
export const unstagePaths = (path: string, paths: string[]) =>
  invoke<void>("unstage_paths", { path, paths });
export const createCommit = (path: string, message: string) =>
  invoke<string>("create_commit", { path, message });

export const watchRepo = (path: string) => invoke<void>("watch_repo", { path });
export const unwatchRepo = () => invoke<void>("unwatch_repo");

export interface SyncStatus {
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  hasRemote: boolean;
}

export const getSyncStatus = (path: string) => invoke<SyncStatus>("get_sync_status", { path });
export const gitFetch = (path: string) => invoke<string>("git_fetch", { path });
export const gitPull = (path: string) => invoke<string>("git_pull", { path });
export const gitPush = (path: string) => invoke<string>("git_push", { path });

export interface BranchInfo {
  name: string;
  isHead: boolean;
  upstream: string | null;
  ahead: number;
  behind: number;
}

export const getLocalBranches = (path: string) =>
  invoke<BranchInfo[]>("get_local_branches", { path });

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

export const termStart = (path: string, id: number, cols: number, rows: number) =>
  invoke<void>("term_start", { path, id, cols, rows });
export const termWrite = (id: number, data: string) => invoke<void>("term_write", { id, data });
export const termResize = (id: number, cols: number, rows: number) =>
  invoke<void>("term_resize", { id, cols, rows });
export const termStop = () => invoke<void>("term_stop");
/** Creates a branch at the current commit and checks it out. */
export const createBranch = (path: string, name: string) =>
  invoke<void>("create_branch", { path, name });

/** Switches to an existing local branch; rejects if local changes would be overwritten. */
export const checkoutLocalBranch = (path: string, name: string) =>
  invoke<void>("checkout_local_branch", { path, name });

/** Number of commits that would be left unreachable by deleting the branch (0 if merged). */
export const countUnmergedCommits = (path: string, name: string) =>
  invoke<number>("count_unmerged_commits", { path, name });
export const deleteLocalBranch = (path: string, name: string) =>
  invoke<void>("delete_local_branch", { path, name });

/** Native yes/no dialog. */
export const confirmDialog = (message: string, title: string, danger = false) =>
  ask(message, {
    title,
    kind: danger ? "warning" : "info",
    okLabel: "Delete",
    cancelLabel: "Cancel",
  });

/** Commits that exist only on origin/<name> (not in HEAD or any local branch). */
export const countUnmergedRemoteCommits = (path: string, name: string) =>
  invoke<number>("count_unmerged_remote_commits", { path, name });
/** Deletes origin/<name> on the server. */
export const deleteRemoteBranch = (path: string, name: string) =>
  invoke<string>("delete_remote_branch", { path, name });
