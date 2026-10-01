import { useCallback, useEffect, useState } from "react";
import { confirmDialog } from "@/api/dialog";
import {
  countUnmergedRemoteCommits,
  deleteRemoteBranch,
  getOrigin,
  type RemoteInfo,
} from "@/api/remotes";
import ContextMenu from "@/components/ContextMenu";
import Section from "@/components/Section";
import { useLatestRequest } from "@/hooks/useLatestRequest";
import AddOrigin from "./AddOrigin";

export default function Remotes({
  path,
  refreshKey,
  onChanged,
}: {
  path: string;
  refreshKey: number;
  onChanged: () => void;
}) {
  const [origin, setOrigin] = useState<RemoteInfo | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const start = useLatestRequest();
  const [menu, setMenu] = useState<{ x: number; y: number; branch: string } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const isCurrent = start();
    try {
      const o = await getOrigin(path);
      if (!isCurrent()) return;
      setOrigin(o);
      setError(null);
    } catch (e) {
      if (isCurrent()) setError(String(e));
    }
  }, [path, start]);

  useEffect(() => {
    setOrigin(undefined);
    setMenu(null);
  }, [path]);

  const deleteBranch = async (name: string) => {
    if (!origin) return;
    const full = `${origin.name}/${name}`;
    try {
      const unique = await countUnmergedRemoteCommits(path, name);
      const warning =
        unique > 0
          ? `

${unique} commit${unique === 1 ? "" : "s"} on it exist nowhere else: not in your current branch or any local branch.`
          : "";
      const ok = await confirmDialog(
        `Delete "${full}" from the remote? The branch is removed on the server for everyone who uses it.${warning}`,
        "Delete remote branch",
        true,
      );
      if (!ok) return;
      setBusy(full);
      setError(null);
      await deleteRemoteBranch(path, name);
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  return (
    <Section title="Remotes" count={origin ? origin.branches.length : undefined}>
      {error && <p className="error side-msg">{error}</p>}
      {busy && <p className="muted side-msg">Deleting {busy}…</p>}
      {origin === null && <AddOrigin path={path} onAdded={onChanged} />}
      {origin && (
        <>
          <div className="remote-head" title={origin.url}>
            <span className="rname">{origin.name}</span>
            <span className="rurl">{origin.url}</span>
          </div>
          {origin.branches.length === 0 && (
            <p className="muted side-msg">No remote branches yet. Fetch to load them.</p>
          )}
          <ul className="branchlist">
            {origin.branches.map((b) => (
              <li
                key={b}
                className={menu?.branch === b ? "ctx" : ""}
                title={`${origin.name}/${b}`}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setMenu({ x: e.clientX, y: e.clientY, branch: b });
                }}
              >
                <span className="bname">{b}</span>
              </li>
            ))}
          </ul>
        </>
      )}
      {menu && origin && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={closeMenu}
          items={[
            {
              label: "Delete remote branch",
              danger: true,
              disabled: busy !== null || origin.trackedByHead === menu.branch,
              title:
                origin.trackedByHead === menu.branch
                  ? "This is the upstream of the checked-out branch. Switch branches first."
                  : undefined,
              onClick: () => deleteBranch(menu.branch),
            },
          ]}
        />
      )}
    </Section>
  );
}
