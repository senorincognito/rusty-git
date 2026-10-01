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

export interface RebaseCommit {
  id: string;
  shortId: string;
  /** Full message, to pre-fill the editor. */
  message: string;
  author: string;
  /** Unix seconds. */
  time: number;
  /** Already on the upstream: changing it rewrites published history. */
  pushed: boolean;
  isMerge: boolean;
}

export interface RebasePlan {
  /** HEAD when the plan was made; applying refuses if the branch moved since. */
  headId: string;
  /** The commits after the base on the current branch, newest first. */
  commits: RebaseCommit[];
}

/** The commits an interactive rebase onto `id` covers (every later commit on the current branch). */
export const getRebasePlan = (path: string, id: string) => invoke<RebasePlan>("get_rebase_plan", { path, id });

export type RebaseAction = "pick" | "reword" | "squash" | "drop";

export interface RebaseStep {
  id: string;
  action: RebaseAction;
  /** The new message of a "reword". */
  message?: string;
}

/**
 * Applies an interactive rebase. A "squash" commit is melded into the commit before it (the next older one)
 * with its message appended; a "reword" gets a new message; commits without a step are picked. Every commit
 * after the oldest change is rebuilt and the branch moves. A "drop" removes the commit and its changes: the later
 * commits are replayed (new ids, new content), which needs a clean working directory and is abandoned, with nothing
 * changed, if one of them depends on a dropped commit.
 */
export const applyRebase = (path: string, baseId: string, headId: string, steps: RebaseStep[]) =>
  invoke<void>("apply_rebase_cmd", { path, baseId, headId, steps });

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

export type ResetMode = "soft" | "mixed" | "hard";

/** What resetting to a commit would do to the branch, the staging area and the files. */
export interface ResetInfo {
  /** The checked-out branch; null on a detached HEAD. */
  branch: string | null;
  fromShort: string;
  targetShort: string;
  targetSummary: string;
  /** The target is the commit HEAD is already on. */
  sameCommit: boolean;
  /** The target is HEAD or one of its ancestors (the branch moves back). */
  isAncestor: boolean;
  /** Commits that would no longer be on the branch, and the newest few of their summaries. */
  removed: number;
  removedSummaries: string[];
  /** Commits the branch would gain. */
  added: number;
  /** How many removed commits are already on the upstream (a force push would be needed). */
  pushedRemoved: number;
  /** Files with uncommitted changes (a hard reset throws these away). */
  workingChanges: number;
}

export const getResetInfo = (path: string, id: string) => invoke<ResetInfo>("get_reset_info", { path, id });

/**
 * git reset --soft / --mixed / --hard to a commit. Soft moves only the branch, mixed also resets
 * the staging area, hard also resets the files (uncommitted changes are lost).
 */
export const resetToCommit = (path: string, id: string, mode: ResetMode) =>
  invoke<void>("reset_to_commit", { path, id, mode });
