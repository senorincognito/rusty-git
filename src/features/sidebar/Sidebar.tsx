import { useState } from "react";
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
  onStashDropped,
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
  /** A stash was deleted from the list (by its commit id). */
  onStashDropped: (id: string) => void;
}) {
  // One filter for every list below: local branches, remote branches and stashes.
  const [filter, setFilter] = useState("");
  return (
    <ResizablePanel edge="right" storageKey="sidebarWidth" defaultWidth={240}>
      <nav className="sidebar">
        <div className="sidefilter">
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape" && filter !== "") {
                e.preventDefault();
                setFilter("");
              }
            }}
            placeholder="Filter branches and stashes…"
            aria-label="Filter branches and stashes"
            spellCheck={false}
            autoComplete="off"
          />
          {filter !== "" && (
            <button className="sidefilter-clear" title="Clear the filter (Esc)" aria-label="Clear the filter" onClick={() => setFilter("")}>
              ×
            </button>
          )}
        </div>
        <LocalBranches path={path} refreshKey={refreshKey} onChanged={onChanged} filter={filter} />
        <Remotes path={path} refreshKey={refreshKey} onChanged={onChanged} fetchError={fetchError} filter={filter} />
        <Stashes
          path={path}
          refreshKey={refreshKey}
          selectedId={selectedId}
          onSelect={onSelectCommit}
          filter={filter}
          onPopped={onStashPopped}
          onDropped={onStashDropped}
        />
      </nav>
    </ResizablePanel>
  );
}
