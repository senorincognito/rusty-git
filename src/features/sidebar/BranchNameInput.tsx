import { useEffect, useRef, useState } from "react";

/**
 * Inline editor that takes the place of a branch name in the sidebar list. Enter confirms,
 * Escape (or clicking away) cancels. `onSubmit` may reject the name; the parent then keeps
 * the editor open so the name can be corrected.
 */
export default function BranchNameInput({
  initial,
  onSubmit,
  onCancel,
  label = "New branch name",
}: {
  initial: string;
  onSubmit: (value: string) => Promise<void>;
  onCancel: () => void;
  /** Accessible name; the editor is also used for the remote URL. */
  label?: string;
}) {
  const [value, setValue] = useState(initial);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const input = useRef<HTMLInputElement>(null);

  // Focus with the whole name selected; focus again after a rejected attempt.
  useEffect(() => {
    if (!busy) {
      input.current?.focus();
      input.current?.select();
    }
  }, [busy]);

  const submit = async () => {
    const name = value.trim();
    if (name === "" || name === initial) return onCancel();
    busyRef.current = true;
    setBusy(true);
    try {
      await onSubmit(name);
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  return (
    <input
      ref={input}
      className="branch-input"
      value={value}
      disabled={busy}
      spellCheck={false}
      autoComplete="off"
      aria-label={label}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          submit();
        } else if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        }
      }}
      // A confirmation dialog steals focus while we wait: don't treat that as clicking away.
      onBlur={() => !busyRef.current && onCancel()}
      // The row checks out the branch on double-click; editing the name must not trigger that.
      onDoubleClick={(e) => e.stopPropagation()}
    />
  );
}
