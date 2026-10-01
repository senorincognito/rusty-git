import { useCallback, useEffect, useState } from "react";
import { getStatus } from "@/api/changes";
import { useLatestRequest } from "@/hooks/useLatestRequest";

/**
 * How many files have uncommitted changes (a file that is both staged and edited counts once).
 * Refreshes with `refreshKey` and when the window regains focus, because edits made in other
 * programs are not watched.
 */
export function useWorkingChangeCount(path: string, refreshKey: number): number {
  const [count, setCount] = useState(0);
  const start = useLatestRequest();

  const refresh = useCallback(async () => {
    const isCurrent = start();
    try {
      const changes = await getStatus(path);
      if (isCurrent()) setCount(changes.length);
    } catch {
      if (isCurrent()) setCount(0);
    }
  }, [path, start]);

  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  useEffect(() => {
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [refresh]);

  return count;
}
