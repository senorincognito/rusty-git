import { invoke } from "@tauri-apps/api/core";

export interface BranchInfo {
  name: string;
  isHead: boolean;
  upstream: string | null;
  ahead: number;
  behind: number;
}

export const getLocalBranches = (path: string) =>
  invoke<BranchInfo[]>("get_local_branches", { path });

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
