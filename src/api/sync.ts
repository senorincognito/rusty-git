import { invoke } from "@tauri-apps/api/core";

export interface SyncStatus {
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  hasRemote: boolean;
}

export const getSyncStatus = (path: string) => invoke<SyncStatus>("get_sync_status", { path });
export const gitFetch = (path: string) => invoke<string>("git_fetch", { path });
export const gitPull = (path: string) => invoke<string>("git_pull", { path });
export const gitPush = (path: string) => invoke<string>("git_push", { path });
/** Overwrites the upstream with the local branch (--force-with-lease). */
export const gitForcePush = (path: string) => invoke<string>("git_force_push", { path });

export interface AutoFetchOutcome {
  /** "ok", "none" (no remotes), "auth" (credentials needed), "offline" or "error". */
  status: "ok" | "none" | "auth" | "offline" | "error";
  message: string;
}

/** Quiet background fetch; never rejects for git failures, reports them in `status`. */
export const autoFetch = (path: string) => invoke<AutoFetchOutcome>("git_auto_fetch", { path });
