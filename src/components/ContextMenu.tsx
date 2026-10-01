import { Fragment, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import "./ContextMenu.scss";

export interface MenuItem {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  /** Shown as a tooltip, e.g. to explain why an item is disabled. */
  title?: string;
  danger?: boolean;
  /** Shows a check mark (or an empty gutter) before the label, like a toggle or radio item. */
  checked?: boolean;
  /** Draws a dividing line above this item. */
  separatorBefore?: boolean;
}

/** Floating menu at a screen position. Closes on outside click, Escape, scroll or blur. */
export default function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const hasChecks = items.some((i) => i.checked !== undefined);
  const [pos, setPos] = useState({ left: x, top: y });

  // Keep the menu inside the window.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    setPos({
      left: Math.max(4, Math.min(x, window.innerWidth - width - 4)),
      top: Math.max(4, Math.min(y, window.innerHeight - height - 4)),
    });
  }, [x, y, items.length]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    window.addEventListener("scroll", onClose, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
      window.removeEventListener("scroll", onClose, true);
    };
  }, [onClose]);

  // Portal to <body>: a popover must float above the layout, whatever container it's declared in.
  return createPortal(
    <div
      className="ctxmenu"
      role="menu"
      ref={ref}
      style={pos}
      onContextMenu={(e) => e.preventDefault()}
    >
      {items.map((item) => (
        <Fragment key={item.label}>
          {item.separatorBefore && <div className="ctxsep" role="separator" />}
          <button
            role={hasChecks ? "menuitemcheckbox" : "menuitem"}
            aria-checked={hasChecks ? !!item.checked : undefined}
            className={"ctxitem" + (item.danger ? " danger" : "")}
            disabled={item.disabled}
            title={item.title}
            onClick={() => {
              onClose();
              item.onClick();
            }}
          >
            {hasChecks && (
              <span className="ctxcheck" aria-hidden="true">
                {item.checked ? "✓" : ""}
              </span>
            )}
            {item.label}
          </button>
        </Fragment>
      ))}
    </div>,
    document.body,
  );
}
