import { useEffect, useId, useRef, useState } from "react";
import { createBranch } from "@/api/branches";

/**
 * Asks for a name, then creates a branch at the current commit and checks it out. Shared by the
 * toolbar's Branch button (a popover) and the Local branches menu (inline), so both behave alike.
 */
export default function NewBranchForm({
  path,
  className,
  onCreated,
  onCancel,
}: {
  path: string;
  className: string;
  onCreated: () => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const id = useId();

  useEffect(() => input.current?.focus(), []);

  const submit = async () => {
    setBusy(true);
    try {
      await createBranch(path, name);
      onCreated();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <form
      className={className}
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
      onKeyDown={(e) => e.key === "Escape" && onCancel()}
    >
      <label htmlFor={id}>New branch name</label>
      <input
        id={id}
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
  );
}
