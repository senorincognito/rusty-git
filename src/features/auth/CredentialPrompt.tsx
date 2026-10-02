import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { answerCredentials } from "@/api/auth";
import Modal from "@/components/Modal";
import { t } from "@/i18n";
import "./CredentialPrompt.scss";

interface Request {
  id: number;
  /** Git's or ssh's own question, e.g. "Username for 'https://github.com': ". */
  prompt: string;
}

/** Shows what git (or ssh) asks for when it needs a username, password, token or passphrase; one at a time. */
export default function CredentialPrompt() {
  const [queue, setQueue] = useState<Request[]>([]);
  const [value, setValue] = useState("");

  useEffect(() => {
    const unlisten = listen<Request>("credentials-request", (e) => setQueue((q) => [...q, e.payload]));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const current = queue[0];
  if (!current) return null;

  const finish = (answer: string | null) => {
    answerCredentials(current.id, answer).catch(() => {});
    setQueue((q) => q.slice(1));
    setValue("");
  };
  const secret = /password|passphrase|token|secret/i.test(current.prompt);

  return (
    <Modal title={t.auth.title} onClose={() => finish(null)} width={460}>
      <form
        className="credential-form"
        onSubmit={(e) => {
          e.preventDefault();
          finish(value);
        }}
      >
        <p>{current.prompt.replace(/:\s*$/, "")}</p>
        <input
          // A new prompt must start empty even though the same input stays mounted.
          key={current.id}
          autoFocus
          type={secret ? "password" : "text"}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          spellCheck={false}
          autoComplete="off"
          aria-label={t.auth.answerLabel}
        />
        {secret && <p className="modal-hint">{t.auth.tokenHint}</p>}
        <div className="modal-actions">
          <button type="button" className="secondary" onClick={() => finish(null)}>
            {t.common.cancel}
          </button>
          <button type="submit" className="primary">
            {t.auth.submit}
          </button>
        </div>
      </form>
    </Modal>
  );
}
