import { useCallback, useEffect, useState } from "react";
import { getStashes, type StashEntry } from "@/api/stash";
import Section from "@/components/Section";
import { useLatestRequest } from "@/hooks/useLatestRequest";

/** The stashes of the repository, newest first. Clicking one shows its changes in the right panel. */
export default function Stashes({
  path,
  refreshKey,
  selectedId,
  onSelect,
}: {
  path: string;
  refreshKey: number;
  /** The commit whose details are open, so the matching stash can be highlighted. */
  selectedId: string | null;
  onSelect: (stash: { id: string; shortId: string }) => void;
}) {
  const [stashes, setStashes] = useState<StashEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const start = useLatestRequest();

  const refresh = useCallback(async () => {
    const isCurrent = start();
    try {
      const list = await getStashes(path);
      if (!isCurrent()) return;
      setStashes(list);
      setError(null);
    } catch (e) {
      if (isCurrent()) setError(String(e));
    }
  }, [path, start]);

  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  return (
    <Section title="Stashes" count={stashes?.length}>
      {error && <p className="error side-msg">{error}</p>}
      {stashes?.length === 0 && <p className="muted side-msg">No stashes.</p>}
      <ul className="branchlist stashlist">
        {stashes?.map((s) => (
          <li
            key={s.id}
            className={s.id === selectedId ? "selected" : ""}
            title={`${s.message}\n${new Date(s.time * 1000).toLocaleString()}`}
            role="button"
            tabIndex={0}
            onClick={() => onSelect({ id: s.id, shortId: s.shortId })}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onSelect({ id: s.id, shortId: s.shortId });
              }
            }}
          >
            <code className="stash-idx">{`stash@{${s.index}}`}</code>
            <span className="bname">{s.message}</span>
          </li>
        ))}
      </ul>
    </Section>
  );
}
