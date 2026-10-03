import { useRef, useState, type ReactNode } from "react";
import { usePersistentState } from "@/hooks/usePersistentState";
import Splitter from "./Splitter";
import "./Section.scss";

const MIN_HEIGHT = 96; // keep in step with .sidebox.open in Section.scss
const isHeight = (v: unknown): v is number | null => v === null || (typeof v === "number" && v >= MIN_HEIGHT);

/**
 * A collapsible box with a title, an optional count badge and an optional "⋯" button in the headline.
 * With `resizeKey` its bottom edge can be dragged to set the height (remembered under that key); the other boxes
 * in the panel keep at least their headline, so the drag can never push one out of view.
 */
export default function Section({
  title,
  resizeKey,
  count,
  action,
  children,
}: {
  title: string;
  resizeKey?: string;
  count?: number;
  /** A "⋯" button at the right end of the headline, e.g. to open a menu below it. */
  action?: { label: string; active?: boolean; onClick: (button: DOMRect) => void };
  children: ReactNode;
}) {
  const [open, setOpen] = useState(true);
  const [height, setHeight] = usePersistentState<number | null>(`sectionHeight.${resizeKey}`, null, isHeight);
  const root = useRef<HTMLElement>(null);
  const drag = useRef({ from: 0, max: 0 });

  const begin = () => {
    const el = root.current;
    const panel = el?.parentElement;
    if (!el || !panel) return;
    // The most this box can take: what is left once everything else in the panel (the filter field, the other boxes
    // at their minimum or, when collapsed, their headline) has its share.
    let others = 0;
    for (const child of Array.from(panel.children) as HTMLElement[]) {
      if (child === el) continue;
      others += child.classList.contains("open") ? MIN_HEIGHT : child.offsetHeight;
    }
    drag.current = { from: el.offsetHeight, max: Math.max(MIN_HEIGHT, panel.clientHeight - others) };
  };
  const resizeTo = (h: number) => setHeight(Math.round(Math.min(Math.max(h, MIN_HEIGHT), drag.current.max)));

  return (
    <section
      ref={root}
      className={"sidebox" + (open ? " open" : "")}
      style={open && height !== null ? { flex: `0 1 ${height}px` } : undefined}
    >
      <div className="sidebox-headrow">
        <button className="sidebox-head" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
          <span className="chev">{open ? "▾" : "▸"}</span>
          <span>{title}</span>
          {count !== undefined && <span className="count">{count}</span>}
        </button>
        {action && (
          <button
            className={"sidebox-action" + (action.active ? " active" : "")}
            title={action.label}
            aria-label={action.label}
            aria-haspopup="menu"
            aria-expanded={!!action.active}
            // Keeps a menu's outside-click handler from closing it right before this toggles it.
            onMouseDown={(e) => e.stopPropagation()}
            onClick={(e) => action.onClick(e.currentTarget.getBoundingClientRect())}
          >
            ⋯
          </button>
        )}
      </div>
      {open && <div className="sidebox-body">{children}</div>}
      {open && resizeKey && (
        <Splitter
          onStart={begin}
          onMove={(dy) => resizeTo(drag.current.from + dy)}
          onNudge={(dy) => resizeTo(drag.current.from + dy)}
          onEnd={() => {}}
          onReset={() => setHeight(null)}
        />
      )}
    </section>
  );
}
