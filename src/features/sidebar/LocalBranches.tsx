import { useCallback, useEffect, useState } from "react";
import {
  checkoutLocalBranch,
  countUnmergedCommits,
  deleteLocalBranch,
  renameLocalBranch,
  getLocalBranches,
  type BranchInfo,
} from "@/api/branches";
import { confirmDialog } from "@/api/dialog";
import ContextMenu from "@/components/ContextMenu";
import Section from "@/components/Section";
import { useLatestRequest } from "@/hooks/useLatestRequest";
import NewBranchForm from "@/features/toolbar/NewBranchForm";
import BranchNameInput from "./BranchNameInput";

export default function LocalBranches({
  path,
  refreshKey,
  onChanged,
}: {
  path: string;
  refreshKey: number;
  onChanged: () => void;
}) {
  const [branches, setBranches] = useState<BranchInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const start = useLatestRequest();

  const refresh = useCallback(async () => {
    const isCurrent = start();
    try {
      const b = await getLocalBranches(path);
      if (!isCurrent()) return;
      setBranches(b);
      setError(null);
    } catch (e) {
      if (isCurrent()) setError(String(e));
    }
  }, [path, start]);

  const [switching, setSwitching] = useState(false);
  const switchTo = async (b: BranchInfo) => {
    if (b.isHead || switching) return;
    setSwitching(true);
    try {
      await checkoutLocalBranch(path, b.name);
      setError(null);
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setSwitching(false);
    }
  };

  const [menu, setMenu] = useState<{ x: number; y: number; branch: BranchInfo } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  // The "⋯" menu in the section's headline, and the inline new-branch form it opens.
  const [sectionMenu, setSectionMenu] = useState<{ x: number; y: number } | null>(null);
  const closeSectionMenu = useCallback(() => setSectionMenu(null), []);
  const [creating, setCreating] = useState(false);
  // The branch whose name is being edited inline.
  const [editing, setEditing] = useState<string | null>(null);

  const renameBranch = async (b: BranchInfo, newName: string) => {
    try {
      await renameLocalBranch(path, b.name, newName);
      setError(null);
      setEditing(null);
      onChanged();
    } catch (e) {
      setError(String(e)); // the editor stays open so the name can be corrected
    }
  };

  const deleteBranch = async (b: BranchInfo) => {
    try {
      const unmerged = await countUnmergedCommits(path, b.name);
      const message =
        unmerged > 0
          ? `"${b.name}" has ${unmerged} commit${unmerged === 1 ? "" : "s"} that are not merged into the current branch or pushed to its upstream. They will be hard to recover once the branch is gone.

Delete "${b.name}" anyway?`
          : `Delete branch "${b.name}"?`;
      if (!(await confirmDialog(message, "Delete branch", unmerged > 0))) return;
      await deleteLocalBranch(path, b.name);
      setError(null);
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  return (
    <Section
      title="Local branches"
      count={branches?.length}
      action={{
        label: "Branch actions",
        active: sectionMenu !== null,
        onClick: (r) => (sectionMenu ? closeSectionMenu() : setSectionMenu({ x: r.left, y: r.bottom + 4 })),
      }}
    >
      {creating && (
        <NewBranchForm
          path={path}
          className="addremote"
          onCreated={() => {
            setCreating(false);
            onChanged();
          }}
          onCancel={() => setCreating(false)}
        />
      )}
      {error && <p className="error side-msg">{error}</p>}
      {branches?.length === 0 && <p className="muted side-msg">No branches yet.</p>}
      <ul className="branchlist">
        {branches?.map((b) => (
          <li
            key={b.name}
            className={(b.isHead ? "current" : "") + (menu?.branch.name === b.name ? " ctx" : "")}
            title={b.isHead ? `${b.name} (current)` : `Double-click to check out ${b.name}`}
            onDoubleClick={() => editing === null && switchTo(b)}
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu({ x: e.clientX, y: e.clientY, branch: b });
            }}
          >
            {editing === b.name ? (
              <BranchNameInput
                initial={b.name}
                onSubmit={(name) => renameBranch(b, name)}
                onCancel={() => setEditing(null)}
              />
            ) : (
              <span className="bname">{b.name}</span>
            )}
            {b.behind > 0 && <span className="sync">↓{b.behind}</span>}
            {b.ahead > 0 && <span className="sync">↑{b.ahead}</span>}
          </li>
        ))}
      </ul>
      {sectionMenu && (
        <ContextMenu
          x={sectionMenu.x}
          y={sectionMenu.y}
          onClose={closeSectionMenu}
          items={[
            {
              label: "New branch…",
              disabled: creating,
              title: "Create a branch at the current commit and check it out",
              onClick: () => {
                setError(null);
                setCreating(true);
              },
            },
          ]}
        />
      )}
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={closeMenu}
          items={[
            {
              label: "Rename branch",
              disabled: menu.branch.isHead,
              title: menu.branch.isHead
                ? "The checked-out branch can't be renamed. Switch to another branch first."
                : undefined,
              onClick: () => {
                setError(null);
                setEditing(menu.branch.name);
              },
            },
            {
              label: "Delete branch",
              danger: true,
              disabled: menu.branch.isHead,
              title: menu.branch.isHead
                ? "The checked-out branch can't be deleted. Switch to another branch first."
                : undefined,
              onClick: () => deleteBranch(menu.branch),
            },
          ]}
        />
      )}
    </Section>
  );
}
