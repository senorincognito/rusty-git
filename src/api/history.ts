import { invoke } from "@tauri-apps/api/core";

export interface RenameInfo {
  /** Full current message, to pre-fill the editor. */
  message: string;
  /** Already on the upstream: renaming it means rewriting published history. */
  pushed: boolean;
  /** Later commits on the branch that get rebuilt along with it. */
  laterCommits: number;
}

export const getRenameInfo = (path: string, id: string) =>
  invoke<RenameInfo>("get_rename_info", { path, id });

/** Changes a commit's message (rewriting later commits as needed); resolves to its new id. */
export const renameCommitMessage = (path: string, id: string, message: string) =>
  invoke<string>("rename_commit_message", { path, id, message });

export interface DropInfo {
  shortId: string;
  summary: string;
  /** Already on the upstream: dropping it rewrites published history (needs a force push). */
  pushed: boolean;
  /** A merge commit: the branch goes back to its first parent. */
  isMerge: boolean;
  /** Commits after it on the branch, which are re-created (new ids) on top of its parent. */
  laterCommits: number;
  /** How many of those are already on the upstream. */
  laterPushed: number;
}

/** What dropping a commit would involve; rejects if that commit can't be dropped. */
export const getDropInfo = (path: string, id: string) => invoke<DropInfo>("get_drop_info", { path, id });

/**
 * Removes a commit from the current branch, re-creating the commits after it on top of its parent.
 * Needs a clean working directory. If a later commit depends on the dropped one, nothing is changed
 * and the rejection says which.
 */
export const dropLatestCommit = (path: string, id: string) => invoke<void>("drop_latest_commit", { path, id });
