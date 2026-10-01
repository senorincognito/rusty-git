import { useState, type ReactNode } from "react";
import "./Section.scss";

/** A collapsible box with a title, an optional count badge and an optional "⋯" button in the headline. */
export default function Section({
  title,
  count,
  action,
  children,
}: {
  title: string;
  count?: number;
  /** A "⋯" button at the right end of the headline, e.g. to open a menu below it. */
  action?: { label: string; active?: boolean; onClick: (button: DOMRect) => void };
  children: ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <section className="sidebox">
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
    </section>
  );
}
