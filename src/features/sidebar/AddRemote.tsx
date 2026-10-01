import { useState } from "react";
import { addRemote } from "@/api/remotes";

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
        {first ? "No remote yet. Add the URL of the remote repository." : "Add another remote repository."}
      </p>
      <input
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Name, e.g. origin or upstream"
        aria-label="Remote name"
        spellCheck={false}
        autoComplete="off"
        autoFocus={!first}
      />
      <input
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder="https://github.com/user/repo.git"
        aria-label="Remote URL"
        spellCheck={false}
        autoComplete="off"
      />
      {error && <p className="error">{error}</p>}
      <div className="addremote-actions">
        <button className="primary" type="submit" disabled={busy || name.trim() === "" || url.trim() === ""}>
          Add remote
        </button>
        {onCancel && (
          <button type="button" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
        )}
      </div>
    </form>
  );
}
