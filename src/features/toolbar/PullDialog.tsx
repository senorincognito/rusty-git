import type { BriefCommit, Divergence } from "@/api/sync";
import Modal from "@/components/Modal";

function CommitList({ title, commits, total }: { title: string; commits: BriefCommit[]; total: number }) {
  return (
    <section>
      <h3>
        {title} <span className="count">{total}</span>
      </h3>
      <ul>
        {commits.map((c) => (
          <li key={c.shortId} title={c.summary}>
            <code>{c.shortId}</code> <span>{c.summary}</span>
          </li>
        ))}
        {total > commits.length && <li className="muted">… and {total - commits.length} more</li>}
      </ul>
    </section>
  );
}

/**
 * Shown when a pull can't fast-forward because both sides have new commits. Merge is the
 * default (Enter): it leaves every commit as it is. Rebase rewrites the local commits.
 */
export default function PullDialog({
  divergence: d,
  onMerge,
  onRebase,
  onCancel,
}: {
  divergence: Divergence;
  onMerge: () => void;
  onRebase: () => void;
  onCancel: () => void;
}) {
  return (
    <Modal title="Branches have diverged" onClose={onCancel} width={640}>
      <p className="modal-lead">
        <strong>{d.branch}</strong> and <strong>{d.upstream}</strong> each have commits the other doesn't, so
        the pull can't simply fast-forward. Choose how to combine them.
      </p>
      <div className="div-cols">
        <CommitList title="↑ Only on your branch" commits={d.ahead} total={d.aheadTotal} />
        <CommitList title={`↓ Only on ${d.upstream}`} commits={d.behind} total={d.behindTotal} />
      </div>
      <p className="modal-hint">
        <strong>Merge</strong> keeps all commits as they are and adds a merge commit (the safest choice).{" "}
        <strong>Rebase</strong> replays your commits on top of {d.upstream}, which gives them new ids. If there are
        conflicts, the pull is cancelled and nothing is changed. Uncommitted changes are set aside and restored.
      </p>
      <div className="modal-actions">
        <button className="secondary" onClick={onCancel}>
          Cancel
        </button>
        <button className="secondary" onClick={onRebase}>
          Rebase
        </button>
        <button className="primary" onClick={onMerge} autoFocus>
          Merge
        </button>
      </div>
    </Modal>
  );
}
