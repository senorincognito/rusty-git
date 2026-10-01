import ResizablePanel from "@/components/ResizablePanel";
import LocalBranches from "./LocalBranches";
import Remotes from "./Remotes";
import Stashes from "./Stashes";

export default function Sidebar({
  path,
  refreshKey,
  onChanged,
  selectedId,
  onSelectCommit,
}: {
  path: string;
  refreshKey: number;
  onChanged: () => void;
  /** The commit whose details are open (highlights the matching stash). */
  selectedId: string | null;
  onSelectCommit: (commit: { id: string; shortId: string }) => void;
}) {
  return (
    <ResizablePanel edge="right" storageKey="sidebarWidth" defaultWidth={240}>
      <nav className="sidebar">
        <LocalBranches path={path} refreshKey={refreshKey} onChanged={onChanged} />
        <Remotes path={path} refreshKey={refreshKey} onChanged={onChanged} />
        <Stashes path={path} refreshKey={refreshKey} selectedId={selectedId} onSelect={onSelectCommit} />
      </nav>
    </ResizablePanel>
  );
}
