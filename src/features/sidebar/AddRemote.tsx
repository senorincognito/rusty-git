import { useState } from "react";
import { addRemote } from "@/api/remotes";
import { t } from "@/i18n";

/** Name and URL of a new remote. `first` is true while the repository has none (the name defaults to origin). */
export default function AddRemote({
  path,
  first,
  onAdded,
  onCancel,
}: {
  path: string;
  first: boolean;
  onAdded: () => void;
  onCancel?: () => void;
}) {
  const [name, setName] = useState(first ? "origin" : "");
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      await addRemote(path, name, url);
      setError(null);
      onAdded();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      className="addremote"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape" && onCancel) {
          e.preventDefault();
          onCancel();
        }
      }}
    >
      <p className="muted">
        {first ? t.addRemote.first : t.addRemote.another}
      </p>
      <input
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder={t.addRemote.namePlaceholder}
        aria-label={t.addRemote.nameLabel}
        spellCheck={false}
        autoComplete="off"
        autoFocus={!first}
      />
      <input
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder={t.addRemote.urlPlaceholder}
        aria-label={t.addRemote.urlLabel}
        spellCheck={false}
        autoComplete="off"
      />
      {error && <p className="error">{error}</p>}
      <div className="addremote-actions">
        <button className="primary" type="submit" disabled={busy || name.trim() === "" || url.trim() === ""}>
          {t.addRemote.submit}
        </button>
        {onCancel && (
          <button type="button" onClick={onCancel} disabled={busy}>
            {t.common.cancel}
          </button>
        )}
      </div>
    </form>
  );
}
