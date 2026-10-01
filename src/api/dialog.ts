import { ask, message, open } from "@tauri-apps/plugin-dialog";
import { t } from "@/i18n";

/** Shows the native folder picker; resolves to null if cancelled. */
export async function pickFolder(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title: t.welcome.pickerTitle });
  return typeof selected === "string" ? selected : null;
}

/** Native yes/no dialog. */
export const confirmDialog = (message: string, title: string, danger = false, okLabel = t.common.delete) =>
  ask(message, {
    title,
    kind: danger ? "warning" : "info",
    okLabel,
    cancelLabel: t.common.cancel,
  });

/** Native error message box. */
export const showError = (text: string, title: string) => message(text, { title, kind: "error" });
