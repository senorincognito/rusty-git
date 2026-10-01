import { useCallback, useState } from "react";

/**
 * useState that survives restarts via localStorage. Stored values that fail `valid` (or can't be
 * read, e.g. storage is blocked) fall back to `initial`; failing to save is silently ignored.
 */
export function usePersistentState<T>(
  key: string,
  initial: T,
  valid: (value: unknown) => value is T = (v): v is T => v !== undefined,
): [T, (value: T) => void] {
  const [value, setValue] = useState<T>(() => {
    try {
      const raw = localStorage.getItem(key);
      if (raw !== null) {
        const parsed: unknown = JSON.parse(raw);
        if (valid(parsed)) return parsed;
      }
    } catch {
      /* unreadable: use the default */
    }
    return initial;
  });

  const set = useCallback(
    (next: T) => {
      setValue(next);
      try {
        localStorage.setItem(key, JSON.stringify(next));
      } catch {
        /* storage unavailable: the setting just won't persist */
      }
    },
    [key],
  );

  return [value, set];
}
