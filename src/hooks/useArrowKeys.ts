import { useEffect, useRef } from "react";

/**
 * Calls `onArrow(-1)` for ArrowUp and `onArrow(1)` for ArrowDown while `enabled`, for the plain keys only. The
 * handler returns true when it used the key (the default scrolling is then prevented). Typing in a field, an open
 * menu or dialog, composition and modifier keys leave the arrows alone, like the other window-level shortcuts.
 */
export function useArrowKeys(enabled: boolean, onArrow: (step: -1 | 1) => boolean) {
  // Always call the latest handler without re-subscribing on every render.
  const latest = useRef(onArrow);
  latest.current = onArrow;

  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.key !== "ArrowUp" && e.key !== "ArrowDown") || e.defaultPrevented || e.isComposing) return;
      if (e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return;
      const target = e.target as HTMLElement | null;
      if (target && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return;
      if (document.querySelector(".ctxmenu, .modal-backdrop")) return;
      if (latest.current(e.key === "ArrowUp" ? -1 : 1)) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [enabled]);
}
