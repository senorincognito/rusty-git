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
  files: CommitFile[];
  totalFiles: number;
  /** The file list was cut off (very large commit). */
  truncated: boolean;
}

export const getCommitDetail = (path: string, id: string) =>
  invoke<CommitDetail>("get_commit_detail", { path, id });

export interface DiffLine {
  /** "hunk" is a hunk header (changes-only view); "note" is e.g. "No newline at end of file". */
  kind: "ctx" | "add" | "del" | "hunk" | "note";
  oldNo: number | null;
  newNo: number | null;
  text: string;
}

export interface FileDiff {
  lines: DiffLine[];
  /** Binary or too large to show. */
  binary: boolean;
  truncated: boolean;
  additions: number;
  deletions: number;
}

/**
 * One file of a commit as a line diff against the first parent. `fullFile` returns the whole
 * file with additions and deletions marked in place; otherwise only the changed hunks.
 */
export const getFileDiff = (
  path: string,
  id: string,
  file: string,
  oldPath: string | null,
  fullFile: boolean,
) => invoke<FileDiff>("get_file_diff", { path, id, file, oldPath, fullFile });
