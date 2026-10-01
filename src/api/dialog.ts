import { ask, open } from "@tauri-apps/plugin-dialog";

/** Shows the native folder picker; resolves to null if cancelled. */
export async function pickFolder(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title: "Open repository" });
  return typeof selected === "string" ? selected : null;
}

/** Native yes/no dialog. */
export const confirmDialog = (message: string, title: string, danger = false) =>
  ask(message, {
    title,
    kind: danger ? "warning" : "info",
    okLabel: "Delete",
    cancelLabel: "Cancel",
  });
