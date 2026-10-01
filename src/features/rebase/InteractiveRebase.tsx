import { useEffect, useRef, useState } from "react";
import { getRebasePlan, rewordCommits, type RebaseCommit, type RebasePlan } from "@/api/history";
import Modal from "@/components/Modal";
import { fill, t } from "@/i18n";
import "./InteractiveRebase.scss";

const dateFmt = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric" });
const summaryOf = (message: string) => message.split("\n", 1)[0];

type Action = "pick" | "reword";

const ACTIONS: { value: Action; label: string; title: string }[] = [
  { value: "pick", label: t.rebase.pick, title: t.rebase.pickHint },
  { value: "reword", label: t.rebase.reword, title: t.rebase.rewordHint },
];

/**
 * The interactive-rebase screen, shown in place of the sidebar and the graph: every commit after `base`
 * on the current branch, newest first, each with an action. "Start rebase" applies them in one go.
 */
export default function InteractiveRebase({
  path,
  base,
  onCancel,
  onApplied,
}: {
  path: string;
  base: { id: string; shortId: string };
  onCancel: () => void;
  onApplied: () => void;
}) {
  const [plan, setPlan] = useState<RebasePlan | null>(null);
  // New messages of the commits set to "reword" (every other commit is a "pick").
  const [reworded, setReworded] = useState<Record<string, string>>({});
  // The commit whose message is being edited in the popup.
  const [editing, setEditing] = useState<RebaseCommit | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let stale = false;
    setPlan(null);
    setReworded({});
    setEditing(null);
    setError(null);
    getRebasePlan(path, base.id)
      .then((p) => !stale && setPlan(p))
      .catch((e) => !stale && setError(String(e)));
    return () => {
      stale = true;
    };
  }, [path, base.id]);

  // Escape cancels the rebase, unless it already means something else (a field, the popup, a menu).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented || e.isComposing || busy) return;
      const target = e.target as HTMLElement | null;
      if (target && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return;
      if (document.querySelector(".ctxmenu, .modal-backdrop")) return;
      onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel, busy]);

  const commits = plan?.commits ?? [];
  const rewordCount = Object.keys(reworded).length;
  // Everything from the oldest reworded commit up to the tip gets a new id.
  const oldest = commits.reduce((at, c, i) => (c.id in reworded ? i : at), -1);
  const rewritten = oldest < 0 ? [] : commits.slice(0, oldest + 1);
  const pushedRewritten = rewritten.filter((c) => c.pushed).length;
  const canStart = !busy && plan !== null && rewordCount > 0;

  const setAction = (c: RebaseCommit, action: Action) => {
    if (action === "reword") {
      setEditing(c); // the commit only becomes "reword" once the popup is confirmed
    } else {
      setReworded((r) => {
        const next = { ...r };
        delete next[c.id];
        return next;
      });
    }
  };

  const start = async () => {
    if (!plan) return;
    setBusy(true);
    setError(null);
    try {
      await rewordCommits(
        path,
        base.id,
        plan.headId,
        Object.entries(reworded).map(([id, message]) => ({ id, message })),
      );
      onApplied();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const cancelButton = (
    <button className="secondary" onClick={onCancel} disabled={busy}>
      {t.rebase.cancel}
    </button>
  );

  return (
    <section className="rebaseview">
      <header className="rebasehead">
        <div className="rebasetitle">
          <strong>{t.rebase.title}</strong>
          <span className="muted">
            {fill(t.rebase.onto, { base: <code>{base.shortId}</code> })}
            {plan && t.rebase.count(commits.length)}
          </span>
        </div>
        {cancelButton}
      </header>

      <div className="rebasebody">
        {!plan && !error && <p className="muted rebase-msg">{t.common.loading}</p>}
        <ol className="rebaselist">
          {commits.map((c) => {
            const action: Action = c.id in reworded ? "reword" : "pick";
            const message = reworded[c.id] ?? c.message;
            return (
              <li key={c.id} className={"rebaserow " + action}>
                <select
                  className="rebase-action"
                  value={action}
                  disabled={busy}
                  aria-label={t.rebase.actionFor(c.shortId)}
                  title={ACTIONS.find((a) => a.value === action)?.title}
                  onChange={(e) => setAction(c, e.target.value as Action)}
                >
                  {ACTIONS.map((a) => (
                    <option key={a.value} value={a.value} title={a.title}>
                      {a.label}
                    </option>
                  ))}
                </select>
                <code className="rebase-id">{c.shortId}</code>
                <span className="rebase-summary" title={message}>
                  {summaryOf(message)}
                </span>
                {action === "reword" && (
                  <button className="ghost rebase-edit" onClick={() => setEditing(c)} disabled={busy}>
                    {t.rebase.editMessage}
                  </button>
                )}
                {c.isMerge && <span className="rebase-tag">{t.rebase.merge}</span>}
                {c.pushed && (
                  <span className="rebase-tag pushed" title={t.rebase.pushedHint}>
                    {t.rebase.pushed}
                  </span>
                )}
                <span className="rebase-author">{c.author}</span>
                <span className="rebase-date">{dateFmt.format(new Date(c.time * 1000))}</span>
              </li>
            );
          })}
        </ol>
        {plan && (
          <div className="rebasebase">
            <code>{base.shortId}</code> <span className="muted">{t.rebase.base}</span>
          </div>
        )}
      </div>

      <footer className="rebasefoot">
        <div className="rebase-info">
          {error && <p className="error">{error}</p>}
          {!error && rewordCount === 0 && plan && (
            <p className="muted">{t.rebase.chooseAction}</p>
          )}
          {rewordCount > 0 && (
            <p className="muted">{t.rebase.reworded(rewordCount, rewritten.length - rewordCount)}</p>
          )}
          {pushedRewritten > 0 && (
            <p className="warn">
              {t.rebase.pushedWarning(pushedRewritten)}
            </p>
          )}
        </div>
        {cancelButton}
        <button className="primary" onClick={start} disabled={!canStart}>
          {busy ? t.rebase.starting : t.rebase.start}
        </button>
      </footer>

      {editing && (
        <RewordDialog
          commit={editing}
          initial={reworded[editing.id] ?? editing.message}
          onCancel={() => setEditing(null)}
          onUpdate={(message) => {
            const id = editing.id;
            setReworded((r) => {
              const next = { ...r };
              // Back to the original message: nothing to reword.
              if (message.trim() === editing.message.trim()) delete next[id];
              else next[id] = message;
              return next;
            });
            setEditing(null);
          }}
        />
      )}
    </section>
  );
}

/** Popup that edits one commit's message for the rebase. */
function RewordDialog({
  commit,
  initial,
  onCancel,
  onUpdate,
}: {
  commit: RebaseCommit;
  initial: string;
  onCancel: () => void;
  onUpdate: (message: string) => void;
}) {
  const [message, setMessage] = useState(initial);
  const box = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    box.current?.focus();
    box.current?.select();
  }, []);

  const canUpdate = message.trim() !== "";

  return (
    <Modal title={t.rebase.rewordTitle(commit.shortId)} onClose={onCancel} width={560}>
      <textarea
        ref={box}
        className="reword-text"
        value={message}
        rows={8}
        aria-label={t.rebase.messageLabel}
        onChange={(e) => setMessage(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && canUpdate) onUpdate(message);
        }}
      />
      {!canUpdate && <p className="error">{t.rebase.emptyMessage}</p>}
      <p className="modal-hint">{t.rebase.rewordAppliesLater}</p>
      <div className="modal-actions">
        <button className="secondary" onClick={onCancel}>
          {t.common.cancel}
        </button>
        <button className="primary" onClick={() => onUpdate(message)} disabled={!canUpdate}>
          {t.rebase.updateMessage}
        </button>
      </div>
    </Modal>
  );
}
