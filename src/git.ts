import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

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
