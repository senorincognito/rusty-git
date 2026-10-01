import { useState } from "react";
import { createStash } from "@/api/stash";
import Modal from "@/components/Modal";

/** Asks for an optional message, then moves all uncommitted changes into a stash. */
export default function StashDialog({
  path,
  fileCount,
  onClose,
  onStashed,
}: {
  path: string;
  fileCount: number;
  onClose: () => void;
  onStashed: () => void;
}) {
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      await createStash(path, message.trim() === "" ? null : message);
      onStashed();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <Modal title="Stash changes" onClose={onClose} width={460}>
      <form
        className="stash-form"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <p>
          {fileCount === 1 ? "The 1 changed file" : `All ${fileCount} changed files`}, including untracked files, will be
          moved into a new stash and your working directory will be clean.
        </p>
        <input
          autoFocus
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="Message (optional)"
          spellCheck={false}
          autoComplete="off"
          aria-label="Stash message"
          disabled={busy}
        />
        {error && <p className="error">{error}</p>}
        <p className="modal-hint">Select the stash in the graph or the Stashes list and press Pop to bring the changes back.</p>
        <div className="modal-actions">
          <button type="button" className="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button type="submit" className="primary" disabled={busy}>
            Stash
          </button>
        </div>
      </form>
    </Modal>
  );
}
