import { invoke } from "@tauri-apps/api/core";

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
