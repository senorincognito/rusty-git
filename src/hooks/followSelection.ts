/**
 * Keeps a list's selected row in view and, when keyboard focus is on one of the list's rows, moves the focus
 * (and with it the focus outline) onto the selected row. Without that, selecting with the arrow keys leaves the
 * outline on the row that was clicked last while the highlight moves on.
 */
export function followSelection(list: HTMLElement | null, selected = ".selected") {
  const row = list?.querySelector<HTMLElement>(selected);
  if (!list || !row) return;
  row.scrollIntoView({ block: "nearest" });
  const active = document.activeElement as HTMLElement | null;
  if (active && active !== row && active.matches("li") && list.contains(active)) row.focus({ preventScroll: true });
}
