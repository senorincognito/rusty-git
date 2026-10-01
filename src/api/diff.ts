import { invoke } from "@tauri-apps/api/core";

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

/**
 * A file's uncommitted changes. `staged` compares HEAD with the index (what the next commit
 * contains); otherwise the index with the file on disk. Untracked files show as all additions.
 */
export const getWorkingDiff = (path: string, file: string, staged: boolean, fullFile: boolean) =>
  invoke<FileDiff>("get_working_diff", { path, file, staged, fullFile });
