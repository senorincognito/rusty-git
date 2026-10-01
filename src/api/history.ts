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
