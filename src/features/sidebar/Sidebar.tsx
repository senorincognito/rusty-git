import ResizablePanel from "@/components/ResizablePanel";
import LocalBranches from "./LocalBranches";
import Remotes from "./Remotes";
import Stashes from "./Stashes";
import "./Sidebar.scss";

export default function Sidebar({
  path,
  refreshKey,
  onChanged,
  fetchError,
  selectedId,
  onSelectCommit,
  onStashPopped,
}: {
  path: string;
  refreshKey: number;
  onChanged: () => void;
  /** Why the last fetch failed, if it did. */
  fetchError: string | null;
  /** The commit whose details are open (highlights the matching stash). */
  selectedId: string | null;
  onSelectCommit: (commit: { id: string; shortId: string }) => void;
  /** A stash was popped from the list (by its commit id). */
  onStashPopped: (id: string) => void;
}) {
  return (
    <ResizablePanel edge="right" storageKey="sidebarWidth" defaultWidth={240}>
      <nav className="sidebar">
        <LocalBranches path={path} refreshKey={refreshKey} onChanged={onChanged} />
        <Remotes path={path} refreshKey={refreshKey} onChanged={onChanged} fetchError={fetchError} />
        <Stashes
          path={path}
          refreshKey={refreshKey}
          selectedId={selectedId}
          onSelect={onSelectCommit}
          onPopped={onStashPopped}
        />
      </nav>
    </ResizablePanel>
  );
}
