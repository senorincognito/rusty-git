import { invoke } from "@tauri-apps/api/core";

export interface CommitFile {
  path: string;
  /** Previous path, for renamed and copied files. */
  oldPath: string | null;
  status: "new" | "modified" | "deleted" | "renamed" | "copied" | "typechange";
}

export interface CommitDetail {
  id: string;
  shortId: string;
  summary: string;
  /** Everything after the first line of the message; empty for one-liners. */
  body: string;
  author: string;
  email: string;
  /** Unix seconds. */
  time: number;
  /** Short ids of the parents. */
  parents: string[];
  /** Changes are relative to the first parent. */
  isMerge: boolean;
  /** "stash@{n}" when this commit is a stash. */
  stash: string | null;
  files: CommitFile[];
  totalFiles: number;
  /** The file list was cut off (very large commit). */
  truncated: boolean;
}

export const getCommitDetail = (path: string, id: string) =>
  invoke<CommitDetail>("get_commit_detail", { path, id });
