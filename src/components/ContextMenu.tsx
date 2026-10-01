import { Fragment, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import "./ContextMenu.scss";

export interface MenuItem {
  label: string;
  /** What the item does. Not used by group items (those with `children`). */
  onClick?: () => void;
  disabled?: boolean;
  /** Shown as a tooltip, e.g. to explain why an item is disabled. */
  title?: string;
  danger?: boolean;
  /** Shows a check mark (or an empty gutter) before the label, like a toggle or radio item. */
  checked?: boolean;
  /** Draws a dividing line above this item. */
  separatorBefore?: boolean;
  /**
   * Makes this a group: the item opens a submenu with these entries, snapped to the right of the
   * menu, on hover, on click or with the right arrow key.
   */
  children?: MenuItem[];
}

const EDGE = 4; // keep this far from the window edge
const HOVER_DELAY = 120; // ms of "hover intent" before a submenu opens or closes, so a diagonal move doesn't flicker
const MENU_PADDING = 5; // padding + border of .ctxmenu: aligns a submenu's first entry with its parent item

type Place =
  | { kind: "point"; x: number; y: number }
  // Snapped to the side of the parent menu, level with the item that opened it.
  | { kind: "snap"; panel: DOMRect; item: DOMRect };

const clamp = (value: number, size: number, limit: number) => Math.max(EDGE, Math.min(value, limit - size - EDGE));

/** One menu panel; group items open another Panel for their children. */
function Panel({
  items,
  place,
  onClose,
  onCloseSelf,
  returnFocus,
  focusFirst,
  onEnter,
}: {
  items: MenuItem[];
  place: Place;
  /** Close the whole menu (an entry was chosen). */
  onClose: () => void;
  /** Close just this panel (submenus only): Escape and the left arrow key. */
  onCloseSelf?: () => void;
  /** Where focus goes back to when a submenu closes by keyboard. */
  returnFocus?: HTMLElement;
  focusFirst?: boolean;
  /** The pointer entered this panel: the parent must not close it any more. */
  onEnter?: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState(() =>
    place.kind === "point" ? { left: place.x, top: place.y } : { left: place.panel.right - 1, top: place.item.top - MENU_PADDING },
  );
  const [open, setOpen] = useState<{ index: number; item: DOMRect; panel: DOMRect; opener: HTMLElement; focus: boolean } | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const hasChecks = items.some((i) => i.checked !== undefined);

  // Place the panel: at the pointer, or snapped to its parent menu; keep it inside the window.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    if (place.kind === "point") {
      setPos({ left: clamp(place.x, width, window.innerWidth), top: clamp(place.y, height, window.innerHeight) });
    } else {
      // To the right of the parent; flip to its left when there is no room.
      const right = place.panel.right - 1;
      const fits = right + width <= window.innerWidth - EDGE;
      setPos({
        left: fits ? right : Math.max(EDGE, place.panel.left - width + 1),
        top: clamp(place.item.top - MENU_PADDING, height, window.innerHeight),
      });
    }
  }, [place, items.length]);

  // A submenu takes Escape for itself (closing only the submenu), before the root menu sees it.
  useEffect(() => {
    if (!onCloseSelf) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopImmediatePropagation();
      returnFocus?.focus();
      onCloseSelf();
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [onCloseSelf, returnFocus]);

  useEffect(() => {
    if (focusFirst) ref.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, [focusFirst]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const later = (fn: () => void) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(fn, HOVER_DELAY);
  };
  const closeSub = () => {
    window.clearTimeout(timer.current);
    setOpen(null);
  };
  const openGroup = (index: number, button: HTMLElement, focus: boolean) => {
    window.clearTimeout(timer.current);
    const panel = ref.current?.getBoundingClientRect();
    if (panel) setOpen({ index, item: button.getBoundingClientRect(), panel, opener: button, focus });
  };

  const openItem = open ? items[open.index] : null;

  // Portal to <body>: a popover must float above the layout, whatever container it's declared in.
  return createPortal(
    <>
      <div
        className={"ctxmenu" + (onCloseSelf ? " sub" : "")}
        role="menu"
        ref={ref}
        style={pos}
        onContextMenu={(e) => e.preventDefault()}
        onMouseEnter={() => {
          window.clearTimeout(timer.current);
          onEnter?.();
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowLeft" && onCloseSelf) {
            e.preventDefault();
            returnFocus?.focus();
            onCloseSelf();
          }
        }}
      >
        {items.map((item, index) => {
          const isGroup = !!item.children?.length;
          return (
            <Fragment key={item.label}>
              {item.separatorBefore && <div className="ctxsep" role="separator" />}
              {/* The row (not the button) listens for the pointer: disabled buttons don't report it. */}
              <div
                className="ctxrow"
                onMouseEnter={(e) => {
                  const button = e.currentTarget.querySelector("button");
                  if (isGroup && !item.disabled && button) later(() => openGroup(index, button, false));
                  else if (open) later(closeSub);
                  else window.clearTimeout(timer.current);
                }}
              >
                <button
                  role={isGroup ? "menuitem" : hasChecks ? "menuitemcheckbox" : "menuitem"}
                  aria-checked={!isGroup && hasChecks ? !!item.checked : undefined}
                  aria-haspopup={isGroup ? "menu" : undefined}
                  aria-expanded={isGroup ? open?.index === index : undefined}
                  className={
                    "ctxitem" + (item.danger ? " danger" : "") + (isGroup && open?.index === index ? " open" : "")
                  }
                  disabled={item.disabled}
                  title={item.title}
                  onClick={(e) => {
                    if (isGroup) openGroup(index, e.currentTarget, false);
                    else {
                      onClose();
                      item.onClick?.();
                    }
                  }}
                  onKeyDown={(e) => {
                    if (isGroup && e.key === "ArrowRight") {
                      e.preventDefault();
                      openGroup(index, e.currentTarget, true);
                    }
                  }}
                >
                  {hasChecks && (
                    <span className="ctxcheck" aria-hidden="true">
                      {item.checked ? "✓" : ""}
                    </span>
                  )}
                  {item.label}
                  {isGroup && (
                    <span className="ctxarrow" aria-hidden="true">
                      ▸
                    </span>
                  )}
                </button>
              </div>
            </Fragment>
          );
        })}
      </div>
      {open && openItem?.children && (
        <Panel
          items={openItem.children}
          place={{ kind: "snap", panel: open.panel, item: open.item }}
          onClose={onClose}
          onCloseSelf={closeSub}
          returnFocus={open.opener}
          focusFirst={open.focus}
          onEnter={() => window.clearTimeout(timer.current)}
        />
      )}
    </>,
    document.body,
  );
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
  const place = useMemo<Place>(() => ({ kind: "point", x, y }), [x, y]);

  useEffect(() => {
    // Anywhere inside any menu panel (the root or a submenu) is "inside".
    const onDown = (e: MouseEvent) => {
      if (!(e.target as Element | null)?.closest?.(".ctxmenu")) onClose();
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

  return <Panel items={items} place={place} onClose={onClose} />;
}
