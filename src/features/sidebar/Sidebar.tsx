import ResizablePanel from "@/components/ResizablePanel";
import LocalBranches from "./LocalBranches";
import Remotes from "./Remotes";

export default function Sidebar({
  path,
  refreshKey,
  onChanged,
}: {
  path: string;
  refreshKey: number;
  onChanged: () => void;
}) {
  return (
    <ResizablePanel edge="right" storageKey="sidebarWidth" defaultWidth={240}>
      <nav className="sidebar">
        <LocalBranches path={path} refreshKey={refreshKey} onChanged={onChanged} />
        <Remotes path={path} refreshKey={refreshKey} onChanged={onChanged} />
      </nav>
    </ResizablePanel>
  );
}
