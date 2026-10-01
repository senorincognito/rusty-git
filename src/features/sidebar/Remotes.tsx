import { useCallback, useEffect, useState } from "react";
import { confirmDialog } from "@/api/dialog";
import {
  countUnmergedRemoteCommits,
  deleteRemote,
  deleteRemoteBranch,
  getRemotes,
  renameRemoteBranch,
  setRemoteUrl,
  setTargetRemote,
  type RemoteInfo,
} from "@/api/remotes";
import ContextMenu from "@/components/ContextMenu";
import Section from "@/components/Section";
import { useLatestRequest } from "@/hooks/useLatestRequest";
import AddRemote from "./AddRemote";
import BranchNameInput from "./BranchNameInput";
import { matchesFilter } from "./filter";

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

export default function Remotes({
  path,
  refreshKey,
  onChanged,
  fetchError,
  filter,
}: {
  path: string;
  refreshKey: number;
  onChanged: () => void;
  /** Why the last fetch failed, if it did: shown as a warning beside each remote's name. */
  fetchError: string | null;
  /** Only branches whose name (or "remote/name") matches are listed; a remote whose own name matches shows all of its branches. */
  filter: string;
}) {
  const [remotes, setRemotes] = useState<RemoteInfo[] | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const start = useLatestRequest();
  // Right-click menus: on a remote branch, or on a remote's header.
  const [menu, setMenu] = useState<{ x: number; y: number; remote: string; branch: string } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const [headMenu, setHeadMenu] = useState<{ x: number; y: number; remote: string } | null>(null);
  const closeHeadMenu = useCallback(() => setHeadMenu(null), []);
  // The "⋯" menu in the section's headline.
  const [sectionMenu, setSectionMenu] = useState<{ x: number; y: number } | null>(null);
  const closeSectionMenu = useCallback(() => setSectionMenu(null), []);
  // What a running server operation is doing, e.g. "Deleting origin/x…".
  const [busy, setBusy] = useState<string | null>(null);
  // Inline editors: a remote branch's name, or a remote's URL.
  const [editing, setEditing] = useState<{ remote: string; branch: string } | null>(null);
  const [editingUrl, setEditingUrl] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const refresh = useCallback(async () => {
    const isCurrent = start();
    try {
      const list = await getRemotes(path);
      if (!isCurrent()) return;
      setRemotes(list);
      setError(null);
    } catch (e) {
      if (isCurrent()) setError(String(e));
    }
  }, [path, start]);

  useEffect(() => {
    setRemotes(undefined);
    setMenu(null);
    setHeadMenu(null);
    setSectionMenu(null);
    setEditing(null);
    setEditingUrl(null);
    setAdding(false);
  }, [path]);

  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  const deleteBranch = async (remote: string, name: string) => {
    const full = `${remote}/${name}`;
    try {
      const unique = await countUnmergedRemoteCommits(path, remote, name);
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
      setBusy(`Deleting ${full}…`);
      setError(null);
      await deleteRemoteBranch(path, remote, name);
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const renameBranch = async (remote: string, name: string, newName: string) => {
    const from = `${remote}/${name}`;
    const to = `${remote}/${newName}`;
    try {
      const ok = await confirmDialog(
        `Rename "${from}" to "${to}"?

This creates ${to} and deletes ${from} on the server, for everyone who uses it. Local branches that track ${from} will be pointed at the new name.

If somebody pushed to ${from} since your last fetch, the rename is refused.`,
        "Rename remote branch",
        true,
        "Rename",
      );
      if (!ok) {
        setEditing(null);
        return;
      }
      setBusy(`Renaming ${from}…`);
      setError(null);
      await renameRemoteBranch(path, remote, name, newName);
      setEditing(null);
      onChanged();
    } catch (e) {
      setError(String(e)); // the editor stays open so the name can be corrected
    } finally {
      setBusy(null);
    }
  };

  const changeUrl = async (remote: string, url: string) => {
    try {
      await setRemoteUrl(path, remote, url);
      setEditingUrl(null);
      setError(null);
      onChanged();
    } catch (e) {
      setError(String(e)); // the editor stays open so the URL can be corrected
    }
  };

  const makeTarget = async (remote: string) => {
    try {
      setError(null);
      await setTargetRemote(path, remote);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const removeRemote = async (r: RemoteInfo) => {
    const lines = [
      `Remove the remote "${r.name}" (${r.url})?`,
      "",
      `Only this repository's settings change: the remote and its ${plural(r.branches.length, "remote-tracking branch", "remote-tracking branches")} are removed here. Nothing on the server is touched.`,
    ];
    if (r.trackingBranches > 0) {
      lines.push(
        "",
        `${plural(r.trackingBranches, "local branch", "local branches")} track${r.trackingBranches === 1 ? "s" : ""} it and will no longer have an upstream.`,
      );
    }
    try {
      if (!(await confirmDialog(lines.join("\n"), "Remove remote", true, "Remove"))) return;
      setError(null);
      await deleteRemote(path, r.name);
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  };

  // With a filter, a remote shows only the branches that match (all of them when its own name matches);
  // remotes with nothing to show are hidden.
  const visible = remotes
    ?.map((r) => ({
      r,
      branches: matchesFilter(filter, r.name) ? r.branches : r.branches.filter((b) => matchesFilter(filter, `${r.name}/${b}`)),
    }))
    .filter(({ r, branches }) => filter.trim() === "" || branches.length > 0 || matchesFilter(filter, r.name));
  const several = (remotes?.length ?? 0) > 1;
  const headRemote = headMenu ? remotes?.find((r) => r.name === headMenu.remote) : undefined;
  const branchRemote = menu ? remotes?.find((r) => r.name === menu.remote) : undefined;

  return (
    <Section
      title="Remotes"
      count={visible?.length}
      action={{
        label: "Remote actions",
        active: sectionMenu !== null,
        onClick: (r) => (sectionMenu ? closeSectionMenu() : setSectionMenu({ x: r.left, y: r.bottom + 4 })),
      }}
    >
      {error && <p className="error side-msg">{error}</p>}
      {busy && <p className="muted side-msg">{busy}</p>}
      {remotes?.length === 0 && <AddRemote path={path} first onAdded={onChanged} />}
      {adding && remotes && remotes.length > 0 && (
        <AddRemote
          path={path}
          first={false}
          onAdded={() => {
            setAdding(false);
            onChanged();
          }}
          onCancel={() => setAdding(false)}
        />
      )}
      {remotes && remotes.length > 0 && visible?.length === 0 && (
        <p className="muted side-msg">No remote branch matches the filter.</p>
      )}
      {visible?.map(({ r, branches }) => (
        <div key={r.name} className="remote">
          <div
            className={"remote-head" + (headMenu?.remote === r.name ? " ctx" : "")}
            title={r.url}
            onContextMenu={(e) => {
              e.preventDefault();
              setHeadMenu({ x: e.clientX, y: e.clientY, remote: r.name });
            }}
          >
            <span className="rname">
              {r.name}
              {several && r.isTarget && (
                <span className="rtarget" title="New branches are pushed to this remote">
                  target
                </span>
              )}
              {fetchError && (
                <span className="rwarn" role="img" title={`Fetching failed: ${fetchError}`} aria-label="Fetching failed">
                  ⚠
                </span>
              )}
            </span>
            {editingUrl === r.name ? (
              <BranchNameInput
                initial={r.url}
                label="Remote URL"
                onSubmit={(url) => changeUrl(r.name, url)}
                onCancel={() => setEditingUrl(null)}
              />
            ) : (
              <span className="rurl">{r.url}</span>
            )}
          </div>
          {branches.length === 0 && r.branches.length === 0 && <p className="muted side-msg">No remote branches yet. Fetch to load them.</p>}
          <ul className="branchlist">
            {branches.map((b) => (
              <li
                key={b}
                className={menu?.remote === r.name && menu.branch === b ? "ctx" : ""}
                title={`${r.name}/${b}`}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setMenu({ x: e.clientX, y: e.clientY, remote: r.name, branch: b });
                }}
              >
                {editing?.remote === r.name && editing.branch === b ? (
                  <BranchNameInput
                    initial={b}
                    onSubmit={(name) => renameBranch(r.name, b, name)}
                    onCancel={() => setEditing(null)}
                  />
                ) : (
                  <span className="bname">{b}</span>
                )}
              </li>
            ))}
          </ul>
        </div>
      ))}
      {sectionMenu && remotes && (
        <ContextMenu
          x={sectionMenu.x}
          y={sectionMenu.y}
          onClose={closeSectionMenu}
          items={[
            {
              label: "Add remote…",
              disabled: adding || remotes.length === 0,
              title: remotes.length === 0 ? "Use the form below" : "Add another remote repository",
              onClick: () => {
                setError(null);
                setAdding(true);
              },
            },
            // With several remotes: where Push publishes new branches.
            ...(remotes.length > 1
              ? [
                  {
                    label: "Target remote",
                    separatorBefore: true,
                    title: "The remote that Push publishes new branches to",
                    children: remotes.map((r) => ({
                      label: r.name,
                      checked: r.isTarget,
                      title: r.url,
                      onClick: () => makeTarget(r.name),
                    })),
                  },
                ]
              : []),
          ]}
        />
      )}
      {headMenu && headRemote && (
        <ContextMenu
          x={headMenu.x}
          y={headMenu.y}
          onClose={closeHeadMenu}
          items={[
            {
              label: "Set as target",
              disabled: headRemote.isTarget,
              title: headRemote.isTarget
                ? "New branches are already pushed to this remote"
                : "Push new branches to this remote (Push on a branch that has no upstream yet)",
              onClick: () => makeTarget(headRemote.name),
            },
            {
              label: "Edit URL",
              onClick: () => {
                setError(null);
                setEditingUrl(headRemote.name);
              },
            },
            {
              label: "Remove remote",
              danger: true,
              separatorBefore: true,
              disabled: busy !== null,
              title: "Remove it from this repository's settings (nothing on the server is touched; asks first)",
              onClick: () => removeRemote(headRemote),
            },
          ]}
        />
      )}
      {menu && branchRemote && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={closeMenu}
          items={[
            {
              label: "Rename remote branch",
              disabled: busy !== null || branchRemote.trackedByHead === menu.branch,
              title:
                branchRemote.trackedByHead === menu.branch
                  ? "This is the upstream of the checked-out branch. Switch branches first."
                  : undefined,
              onClick: () => {
                setError(null);
                setEditing({ remote: menu.remote, branch: menu.branch });
              },
            },
            {
              label: "Delete remote branch",
              danger: true,
              disabled: busy !== null || branchRemote.trackedByHead === menu.branch,
              title:
                branchRemote.trackedByHead === menu.branch
                  ? "This is the upstream of the checked-out branch. Switch branches first."
                  : undefined,
              onClick: () => deleteBranch(menu.remote, menu.branch),
            },
          ]}
        />
      )}
    </Section>
  );
}
