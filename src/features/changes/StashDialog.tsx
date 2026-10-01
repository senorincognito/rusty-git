import { useState } from "react";
import { createStash } from "@/api/stash";
import Modal from "@/components/Modal";
import { t } from "@/i18n";
import "./StashDialog.scss";

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
    <Modal title={t.stashDialog.title} onClose={onClose} width={460}>
      <form
        className="stash-form"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <p>{t.stashDialog.explain(fileCount)}</p>
        <input
          autoFocus
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder={t.stashDialog.messagePlaceholder}
          spellCheck={false}
          autoComplete="off"
          aria-label={t.stashDialog.messageLabel}
          disabled={busy}
        />
        {error && <p className="error">{error}</p>}
        <p className="modal-hint">{t.stashDialog.hint}</p>
        <div className="modal-actions">
          <button type="button" className="secondary" onClick={onClose} disabled={busy}>
            {t.common.cancel}
          </button>
          <button type="submit" className="primary" disabled={busy}>
            {t.stashDialog.submit}
          </button>
        </div>
      </form>
    </Modal>
  );
}
