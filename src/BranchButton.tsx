import { useEffect, useRef, useState } from "react";
import { createBranch } from "./git";

export default function BranchButton({
  path,
  onCreated,
}: {
  path: string;
  onCreated: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);

  const close = () => {
    setOpen(false);
    setName("");
    setError(null);
  };

  useEffect(() => {
    if (!open) return;
    input.current?.focus();
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) close();
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  // A different repo means a different context: drop any half-typed name.
  useEffect(close, [path]);

  const submit = async () => {
    setBusy(true);
    try {
      await createBranch(path, name);
      close();
      onCreated();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="branchbtn" ref={root}>
      <button className="syncbtn" onClick={() => (open ? close() : setOpen(true))}>
        ⑂ Branch
      </button>
      {open && (
        <form
          className="branchpop"
          onSubmit={(e) => {
            e.preventDefault();
            submit();
          }}
          onKeyDown={(e) => e.key === "Escape" && close()}
        >
          <label htmlFor="new-branch-name">New branch name</label>
          <input
            id="new-branch-name"
            ref={input}
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setError(null);
            }}
            placeholder="feature/my-change"
            spellCheck={false}
            autoComplete="off"
          />
          {error && <p className="error">{error}</p>}
          <p className="muted hint">Created from the current commit and checked out.</p>
          <button className="primary" type="submit" disabled={busy || name.trim() === ""}>
            Create &amp; checkout
          </button>
        </form>
      )}
    </div>
  );
}
