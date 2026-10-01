import { useState } from "react";
import { addOriginRemote } from "@/api/remotes";

export default function AddOrigin({ path, onAdded }: { path: string; onAdded: () => void }) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      await addOriginRemote(path, url);
      setError(null);
      setUrl("");
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
    >
      <p className="muted">No origin yet. Add the URL of the remote repository.</p>
      <input
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder="https://github.com/user/repo.git"
        spellCheck={false}
        autoComplete="off"
      />
      {error && <p className="error">{error}</p>}
      <button className="primary" type="submit" disabled={busy || url.trim() === ""}>
        Add origin
      </button>
    </form>
  );
}
