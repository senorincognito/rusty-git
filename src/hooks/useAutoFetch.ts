import { useCallback, useEffect, useRef, useState } from "react";
import { autoFetch } from "@/api/sync";

/**
 * - off:      auto-fetch is switched off
 * - idle:     waiting for the next run
 * - fetching: a background fetch is running
 * - auth:     paused because the remote wants credentials (resumes after a successful manual fetch)
 * - waiting:  the remote can't be reached; retrying with a growing delay
 */
export type AutoFetchState = "off" | "idle" | "fetching" | "auth" | "waiting";

const CHECK_EVERY_MS = 10_000; // cheap local check; the fetch itself only runs when due
const FIRST_RUN_DELAY_MS = 3_000;
const MAX_BACKOFF_MS = 30 * 60_000;

interface Schedule {
  last: number; // when the last run finished (0 = never)
  failures: number;
  authPaused: boolean;
  inFlight: boolean;
}
const freshSchedule = (): Schedule => ({ last: 0, failures: 0, authPaused: false, inFlight: false });

/**
 * Background `git fetch` for the open repository, the polite way:
 * only while the window is focused and visible, never overlapping another git operation
 * (`blocked`), fetching right away on focus if the last run is stale, backing off when the
 * remote is unreachable, and stopping instead of retrying when credentials are needed.
 */
export function useAutoFetch({
  path,
  enabled,
  seconds,
  blocked,
}: {
  path: string;
  enabled: boolean;
  seconds: number;
  blocked: boolean;
}) {
  const [state, setState] = useState<AutoFetchState>(enabled ? "idle" : "off");
  const [lastFetchedAt, setLastFetchedAt] = useState<number | null>(null);
  const blockedRef = useRef(blocked);
  blockedRef.current = blocked;
  const sched = useRef<Schedule>(freshSchedule());

  useEffect(() => {
    const s = freshSchedule();
    sched.current = s;
    setLastFetchedAt(null);
    setState(enabled ? "idle" : "off");
    if (!enabled) return;

    let cancelled = false;
    const tick = async () => {
      if (cancelled || s.inFlight || s.authPaused || blockedRef.current) return;
      if (document.visibilityState !== "visible" || !document.hasFocus()) return;
      const wait = Math.min(seconds * 1000 * 2 ** Math.min(s.failures, 3), MAX_BACKOFF_MS);
      if (s.last && Date.now() - s.last < wait) return;

      s.inFlight = true;
      setState("fetching");
      let status: string;
      try {
        status = (await autoFetch(path)).status;
      } catch {
        status = "error";
      }
      s.inFlight = false;
      if (cancelled) return;
      s.last = Date.now();

      if (status === "ok" || status === "none") {
        s.failures = 0;
        if (status === "ok") setLastFetchedAt(s.last);
        setState("idle");
      } else if (status === "auth") {
        s.authPaused = true;
        setState("auth");
      } else {
        s.failures += 1;
        setState("waiting");
      }
    };

    const first = setTimeout(tick, FIRST_RUN_DELAY_MS);
    const timer = setInterval(tick, CHECK_EVERY_MS);
    window.addEventListener("focus", tick);
    document.addEventListener("visibilitychange", tick);
    return () => {
      cancelled = true;
      clearTimeout(first);
      clearInterval(timer);
      window.removeEventListener("focus", tick);
      document.removeEventListener("visibilitychange", tick);
    };
  }, [path, enabled, seconds]);

  /** Call after a manual fetch/pull/push succeeded: credentials and connectivity work again. */
  const resume = useCallback(() => {
    const s = sched.current;
    s.authPaused = false;
    s.failures = 0;
    setState((prev) => (prev === "auth" || prev === "waiting" ? "idle" : prev));
  }, []);

  return { state, lastFetchedAt, resume };
}
