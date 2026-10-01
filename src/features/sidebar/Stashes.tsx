import { useCallback, useEffect, useState } from "react";
import { confirmDialog } from "@/api/dialog";
import { dropStash, getStashes, popStash, type StashEntry } from "@/api/stash";
import ContextMenu from "@/components/ContextMenu";
import Section from "@/components/Section";
import { useLatestRequest } from "@/hooks/useLatestRequest";
import { matchesFilter } from "./filter";
import { useWorkingChangeCount } from "@/hooks/useWorkingChangeCount";

/**
 * The stashes of the repository, newest first. Clicking one shows its changes in the right panel;
 * right-clicking offers to pop or delete it.
 */
export default function Stashes({
  path,
  refreshKey,
  selectedId,
  onSelect,
  onPopped,
  onDropped,
  filter,
}: {
  path: string;
  refreshKey: number;
  /** The commit whose details are open, so the matching stash can be highlighted. */
  selectedId: string | null;
  onSelect: (stash: { id: string; shortId: string }) => void;
  /** A stash was applied and removed (by its commit id). */
  onPopped: (id: string) => void;
  /** A stash was deleted without being applied (by its commit id). */
  onDropped: (id: string) => void;
  /** Only stashes whose message or "stash@{n}" matches are listed. */
  filter: string;
}) {
  const [stashes, setStashes] = useState<StashEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; stash: StashEntry } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const [popping, setPopping] = useState(false);
  const shown = stashes?.filter((s) => matchesFilter(filter, s.message, `stash@{${s.index}}`));
  const workingChanges = useWorkingChangeCount(path, refreshKey);
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

  const pop = async (stash: StashEntry) => {
    setPopping(true);
    setError(null);
    try {
      await popStash(path, stash.id);
      onPopped(stash.id);
    } catch (e) {
      setError(String(e));
    } finally {
      setPopping(false);
    }
  };

  const drop = async (stash: StashEntry) => {
    const ok = await confirmDialog(
      `Delete stash@{${stash.index}} "${stash.message}"?

Its changes are not applied, and they are lost: git has no undo for this.`,
      "Delete stash",
      true,
      "Delete",
    );
    if (!ok) return;
    setPopping(true);
    setError(null);
    try {
      await dropStash(path, stash.id);
      onDropped(stash.id);
    } catch (e) {
      setError(String(e));
    } finally {
      setPopping(false);
    }
  };

  return (
    <Section title="Stashes" count={shown?.length}>
      {error && <p className="error side-msg">{error}</p>}
      {stashes?.length === 0 && <p className="muted side-msg">No stashes.</p>}
      {stashes && stashes.length > 0 && shown?.length === 0 && <p className="muted side-msg">No stash matches the filter.</p>}
      <ul className="branchlist stashlist">
        {shown?.map((s) => (
          <li
            key={s.id}
            className={(s.id === selectedId ? "selected" : "") + (menu?.stash.id === s.id ? " ctx" : "")}
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
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu({ x: e.clientX, y: e.clientY, stash: s });
            }}
          >
            <code className="stash-idx">{`stash@{${s.index}}`}</code>
            <span className="bname">{s.message}</span>
          </li>
        ))}
      </ul>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={closeMenu}
          items={[
            {
              label: "Pop stash",
              disabled: popping || workingChanges > 0,
              title:
                workingChanges > 0
                  ? "Commit or stash your uncommitted changes first"
                  : "Apply this stash to the working directory and remove it from the list",
              onClick: () => pop(menu.stash),
            },
            {
              label: "Delete stash",
              danger: true,
              disabled: popping,
              separatorBefore: true,
              title: "Remove this stash without applying it (asks first)",
              onClick: () => drop(menu.stash),
            },
          ]}
        />
      )}
    </Section>
  );
}
