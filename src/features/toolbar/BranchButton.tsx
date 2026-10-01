import { useEffect, useRef, useState } from "react";
import { t } from "@/i18n";
import NewBranchForm from "./NewBranchForm";
import "./Toolbar.scss";

export default function BranchButton({
  path,
  onCreated,
}: {
  path: string;
  onCreated: () => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  // A different repo means a different context: drop any half-typed name.
  useEffect(() => setOpen(false), [path]);

  return (
    <div className="branchbtn" ref={root}>
      <button className="syncbtn" onClick={() => setOpen((o) => !o)}>
        {t.newBranch.button}
      </button>
      {open && (
        <NewBranchForm
          path={path}
          className="branchpop"
          onCreated={() => {
            setOpen(false);
            onCreated();
          }}
          onCancel={() => setOpen(false)}
        />
      )}
    </div>
  );
}
