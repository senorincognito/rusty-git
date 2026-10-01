import { useEffect, useState } from "react";
import { getRebasePlan, rewordCommits, type RebasePlan } from "@/api/history";
import "./InteractiveRebase.scss";

const dateFmt = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric" });
const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/**
 * Right-hand panel for an interactive rebase onto `base`: every commit after it on the current branch,
 * newest first, each with its message in an editor. For now the only action is rewording; "Apply"
 * rewrites the changed commits (and the ones after them) in one go.
 */
export default function InteractiveRebase({
  path,
  base,
  onClose,
  onApplied,
}: {
  path: string;
  base: { id: string; shortId: string };
  onClose: () => void;
  onApplied: () => void;
}) {
  const [plan, setPlan] = useState<RebasePlan | null>(null);
  // The edited messages by commit id (absent = unchanged).
  const [messages, setMessages] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let stale = false;
    setPlan(null);
    setMessages({});
    setError(null);
    getRebasePlan(path, base.id)
      .then((p) => !stale && setPlan(p))
      .catch((e) => !stale && setError(String(e)));
    return () => {
      stale = true;
    };
  }, [path, base.id]);

  const commits = plan?.commits ?? [];
  const isChanged = (id: string, original: string) => id in messages && messages[id].trim() !== original.trim();
  const changed = commits.filter((c) => isChanged(c.id, c.message));
  const anyEmpty = changed.some((c) => messages[c.id].trim() === "");
  // Everything from the oldest changed commit up to the tip gets a new id.
  const oldestChanged = commits.reduce((at, c, i) => (isChanged(c.id, c.message) ? i : at), -1);
  const rewritten = oldestChanged < 0 ? [] : commits.slice(0, oldestChanged + 1);
  const pushedRewritten = rewritten.filter((c) => c.pushed).length;
  const canApply = !busy && plan !== null && changed.length > 0 && !anyEmpty;

  const apply = async () => {
    if (!plan) return;
    setBusy(true);
    try {
      await rewordCommits(
        path,
        base.id,
        plan.headId,
        changed.map((c) => ({ id: c.id, message: messages[c.id] })),
      );
      onApplied();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside
      className="rebasebox"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
        else if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && canApply) apply();
      }}
    >
      <header className="panel-head">
        <span>Interactive rebase</span>
        <code title="Base: the commits after this one can be changed">onto {base.shortId}</code>
      </header>
      <div className="rebasebody">
        {plan && (
          <p className="note">
            {plural(commits.length, "commit")} after {base.shortId}, newest first. Edit the messages you want to change.
          </p>
        )}
        {!plan && !error && <p className="note">Loading…</p>}
        <ol className="rebaselist">
          {commits.map((c) => {
            const value = messages[c.id] ?? c.message;
            const dirty = isChanged(c.id, c.message);
            return (
              <li key={c.id} className={dirty ? "changed" : ""}>
                <div className="rebase-meta">
                  <code>{c.shortId}</code>
                  <span className="rebase-author">{c.author}</span>
                  <span className="rebase-date">{dateFmt.format(new Date(c.time * 1000))}</span>
                  {c.isMerge && <span className="rebase-tag">merge</span>}
                  {c.pushed && (
                    <span className="rebase-tag pushed" title="Already pushed: changing it needs a force push">
                      pushed
                    </span>
                  )}
                  {dirty && (
                    <button
                      className="ghost rebase-undo"
                      title="Put the original message back"
                      onClick={() =>
                        setMessages((m) => {
                          const next = { ...m };
                          delete next[c.id];
                          return next;
                        })
                      }
                    >
                      Undo
                    </button>
                  )}
                </div>
                <textarea
                  value={value}
                  aria-label={`Message of ${c.shortId}`}
                  rows={Math.min(Math.max(value.split("\n").length, 2), 8)}
                  disabled={busy}
                  onChange={(e) => setMessages((m) => ({ ...m, [c.id]: e.target.value }))}
                />
              </li>
            );
          })}
        </ol>
      </div>
      <footer className="rebasefoot">
        {anyEmpty && <p className="error">A commit message can't be empty.</p>}
        {rewritten.length > changed.length && (
          <p className="note">
            {plural(rewritten.length - changed.length, "later commit")} will be rewritten too (new ids, same content).
          </p>
        )}
        {pushedRewritten > 0 && (
          <p className="warn">
            {plural(pushedRewritten, "rewritten commit")} {pushedRewritten === 1 ? "is" : "are"} already pushed: this
            rewrites published history and will need a force push.
          </p>
        )}
        {error && <p className="error">{error}</p>}
        <div className="rebase-actions">
          <button className="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button className="primary" onClick={apply} disabled={!canApply}>
            {busy ? "Rewording…" : changed.length > 0 ? `Reword ${plural(changed.length, "commit")}` : "Reword"}
          </button>
        </div>
      </footer>
    </aside>
  );
}
