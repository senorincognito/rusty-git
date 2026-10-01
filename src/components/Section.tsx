import { useState, type ReactNode } from "react";
import "./Section.scss";

/** A collapsible box with a title and an optional count badge. */
export default function Section({
  title,
  count,
  children,
}: {
  title: string;
  count?: number;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <section className="sidebox">
      <button className="sidebox-head" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        <span className="chev">{open ? "▾" : "▸"}</span>
        <span>{title}</span>
        {count !== undefined && <span className="count">{count}</span>}
      </button>
      {open && <div className="sidebox-body">{children}</div>}
    </section>
  );
}
