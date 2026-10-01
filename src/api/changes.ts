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
/**
 * Throws away the unstaged changes of whole files: modified and deleted files go back to the staged
 * version, untracked files are deleted. Not undoable.
 */
export const discardPaths = (path: string, paths: string[]) => invoke<void>("discard_paths", { path, paths });
/** Commits the index; with `amend`, replaces the last commit instead. */
export const createCommit = (path: string, message: string, amend = false) =>
  invoke<string>("create_commit", { path, message, amend });

export interface HeadCommit {
  shortId: string;
  message: string;
  /** Already on the branch's upstream: amending it means rewriting published history. */
  pushed: boolean;
}

/** The last commit, or null on a branch without commits. */
export const getHeadCommit = (path: string) => invoke<HeadCommit | null>("get_head_commit", { path });
