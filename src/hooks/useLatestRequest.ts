import { useCallback, useRef } from "react";

/**
 * Guards against out-of-order async responses. Call the returned function when starting a
 * request; it gives back a check that stays true only until a newer request has started:
 *
 *   const start = useLatestRequest();
 *   const refresh = useCallback(async () => {
 *     const isCurrent = start();
 *     const data = await load();
 *     if (isCurrent()) setData(data);
 *   }, [start]);
 */
export function useLatestRequest() {
  const latest = useRef(0);
  return useCallback(() => {
    const id = ++latest.current;
    return () => id === latest.current;
  }, []);
}
