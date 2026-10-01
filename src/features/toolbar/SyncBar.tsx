import { useCallback, useEffect, useState } from "react";
import { confirmDialog } from "@/api/dialog";
import {
  getDivergence,
  getSyncStatus,
  gitFetch,
  gitForcePush,
  gitPull,
  gitPullWith,
  gitPush,
  type Divergence,
  type SyncStatus,
} from "@/api/sync";
import ContextMenu, { type MenuItem } from "@/components/ContextMenu";
import { useAutoFetch } from "@/hooks/useAutoFetch";
import { useLatestRequest } from "@/hooks/useLatestRequest";
import { usePersistentState } from "@/hooks/usePersistentState";
import PullDialog from "./PullDialog";

type Op = "fetch" | "pull" | "push" | "force";
type MenuKind = "fetch" | "pull" | "push";
type Notice = { kind: "ok" | "error"; text: string };

const OPS: Record<Op, { label: string; run: (path: string) => Promise<string> }> = {
  fetch: { label: "Fetch", run: gitFetch },
  pull: { label: "Pull", run: gitPull },
  push: { label: "Push", run: gitPush },
  force: { label: "Force push", run: gitForcePush },
};

// What git says when a pull can't fast-forward.
const isNotFastForward = (msg: string) => /not possible to fast-forward|diverging branches/i.test(msg);

const AUTO_FETCH_CHOICES = [60, 180, 300, 600]; // seconds
const intervalLabel = (secs: number) => `${secs / 60} minute${secs === 60 ? "" : "s"}`;

const AUTO_STATE_HINT = {
  fetching: { icon: "⟳", text: "Fetching in the background…" },
  auth: {
    icon: "⚠",
    text: "Auto-fetch is paused: the remote needs you to sign in. A successful manual fetch resumes it.",
  },
  waiting: {
    icon: "…",
    text: "Auto-fetch can't reach the remote right now. It will retry with a longer delay.",
  },
} as const;

export default function SyncBar({ path, refreshKey }: { path: string; refreshKey: number }) {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [busy, setBusy] = useState<Op | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; kind: MenuKind } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  // Set while the user has to choose how to combine diverged branches.
  const [divergence, setDivergence] = useState<Divergence | null>(null);
  const start = useLatestRequest();

  // Auto-fetch: on by default, every 3 minutes. Global settings, remembered between sessions.
  const [autoOn, setAutoOn] = usePersistentState("autoFetch.enabled", true, (v): v is boolean => typeof v === "boolean");
  const [autoSecs, setAutoSecs] = usePersistentState(
    "autoFetch.seconds",
    180,
    (v): v is number => typeof v === "number" && Number.isFinite(v) && v >= 30,
  );
  const auto = useAutoFetch({ path, enabled: autoOn, seconds: autoSecs, blocked: busy !== null });

  const refresh = useCallback(async () => {
    const isCurrent = start();
    try {
      const s = await getSyncStatus(path);
      if (isCurrent()) setStatus(s);
    } catch {
      if (isCurrent()) setStatus(null);
    }
  }, [path, start]);

  // Reload when the repo changes (a fetch updates remote refs, which the watcher reports).
  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  useEffect(() => {
    setNotice(null);
  }, [path]);

  // Successful results fade away; errors stay until dismissed.
  useEffect(() => {
    if (notice?.kind !== "ok") return;
    const t = setTimeout(() => setNotice(null), 5000);
    return () => clearTimeout(t);
  }, [notice]);

  const run = async (op: Op) => {
    setBusy(op);
    setNotice(null);
    try {
      const out = await OPS[op].run(path);
      setNotice({ kind: "ok", text: out || `${OPS[op].label} complete` });
      auto.resume(); // the remote works and credentials are fine
    } catch (e) {
      setNotice({ kind: "error", text: String(e) });
    } finally {
      setBusy(null);
      refresh();
    }
  };

  const openDivergence = async () => {
    try {
      setDivergence(await getDivergence(path));
    } catch (e) {
      setNotice({ kind: "error", text: String(e) });
    }
  };

  // Pull fast-forwards. When the branches have diverged (known beforehand, or discovered by the
  // pull's own fetch) the user chooses between merge and rebase instead of seeing an error.
  const pull = async () => {
    if (status && status.ahead > 0 && status.behind > 0) return openDivergence();
    setBusy("pull");
    setNotice(null);
    try {
      const out = await gitPull(path);
      setNotice({ kind: "ok", text: out || "Pull complete" });
      auto.resume();
    } catch (e) {
      const text = String(e);
      if (isNotFastForward(text)) {
        await refresh();
        await openDivergence();
      } else {
        setNotice({ kind: "error", text });
      }
    } finally {
      setBusy(null);
      refresh();
    }
  };

  const pullWith = async (mode: "merge" | "rebase") => {
    setDivergence(null);
    setBusy("pull");
    setNotice(null);
    try {
      const out = await gitPullWith(path, mode);
      setNotice({ kind: "ok", text: out || "Pull complete" });
      auto.resume();
    } catch (e) {
      setNotice({ kind: "error", text: String(e) });
    } finally {
      setBusy(null);
      refresh();
    }
  };

  const forcePush = async () => {
    if (!status?.upstream) return;
    const behind = status.behind;
    const message = [
      `Force push "${status.branch}" to ${status.upstream}?`,
      "",
      "This overwrites the remote branch with your local history.",
      behind > 0
        ? `${behind} commit${behind === 1 ? "" : "s"} on ${status.upstream} that are not in your branch will be discarded there.`
        : "",
      "",
      "The push is refused if the remote changed since your last fetch.",
    ]
      .filter((line, i, all) => line !== "" || (i > 0 && all[i - 1] !== ""))
      .join("\n");
    if (await confirmDialog(message, "Force push", true, "Force push")) await run("force");
  };

  const noRemote = status ? !status.hasRemote : false;
  const noBranch = status ? status.branch === null : true;
  // Manual operations wait while a background fetch runs, so two fetches never collide.
  const locked = busy !== null || auto.state === "fetching";
  // A force push in progress is shown on the Push button it came from.
  const shown = (op: Op) => busy === op || (op === "push" && busy === "force");
  const btn = (op: Op, disabled: boolean, title: string, badge?: string, action: () => void = () => run(op)) => (
    <button className="syncbtn" disabled={locked || disabled} title={title} onClick={action}>
      {shown(op) ? `${OPS[op].label}…` : OPS[op].label}
      {badge && !shown(op) && <span className="badge-count">{badge}</span>}
    </button>
  );

  // A button with a small arrow (and right-click) that opens a menu of related actions.
  const withMenu = (kind: MenuKind, label: string, button: React.ReactNode) => (
    <div
      className="splitbtn"
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY, kind });
      }}
    >
      {button}
      {/* stopPropagation keeps the menu's outside-click handler from closing it right before this toggles it */}
      <button
        className="syncbtn splitarrow"
        aria-haspopup="menu"
        aria-expanded={menu?.kind === kind}
        title={label}
        disabled={busy !== null}
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          if (menu?.kind === kind) return closeMenu();
          const r = e.currentTarget.getBoundingClientRect();
          setMenu({ x: r.left, y: r.bottom + 4, kind });
        }}
      >
        ▾
      </button>
    </div>
  );

  const fetchMenuItems: MenuItem[] = [
    {
      label: "Auto-fetch",
      checked: autoOn,
      title: "Fetch in the background while this window is focused",
      onClick: () => setAutoOn(!autoOn),
    },
    ...AUTO_FETCH_CHOICES.map((secs, i) => ({
      label: `Every ${intervalLabel(secs)}`,
      checked: autoSecs === secs,
      disabled: !autoOn,
      separatorBefore: i === 0,
      onClick: () => setAutoSecs(secs),
    })),
  ];

  // Pull with an explicit strategy, skipping the "diverged" dialog.
  const pullDisabledReason = noRemote
    ? "No remotes configured"
    : noBranch
      ? "Check out a branch first"
      : !status?.upstream
        ? "No upstream branch to pull from"
        : null;
  const pullMenuItems: MenuItem[] = [
    {
      label: "Pull (merge)",
      disabled: locked || pullDisabledReason !== null,
      title:
        pullDisabledReason ??
        `Fetch and merge ${status?.upstream} into this branch. A merge commit is added if both have new commits.`,
      onClick: () => pullWith("merge"),
    },
    {
      label: "Pull (rebase)",
      disabled: locked || pullDisabledReason !== null,
      title:
        pullDisabledReason ??
        `Fetch and replay your commits on top of ${status?.upstream}. Your commits get new ids.`,
      onClick: () => pullWith("rebase"),
    },
  ];

  const pushMenuItems: MenuItem[] = [
    {
      label: "Force push",
      danger: true,
      disabled: busy !== null || noRemote || noBranch || !status?.upstream,
      title: noRemote
        ? "No remotes configured"
        : noBranch
          ? "Check out a branch first"
          : !status?.upstream
            ? "This branch has not been pushed yet. Use Push first."
            : `Overwrite ${status.upstream} with your branch (asks first)`,
      onClick: forcePush,
    },
  ];

  const fetchTitle = noRemote
    ? "No remotes configured"
    : "Fetch all remotes" +
      (autoOn
        ? ` · auto-fetch every ${intervalLabel(autoSecs)}` +
          (auto.lastFetchedAt
            ? `, last at ${new Date(auto.lastFetchedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
            : "")
        : " · auto-fetch is off");
  const hint = auto.state === "fetching" || auto.state === "auth" || auto.state === "waiting" ? AUTO_STATE_HINT[auto.state] : null;

  return (
    <>
      <div className="syncbar">
        {hint && (
          <span className={`autofetch ${auto.state}`} title={hint.text} aria-label={hint.text}>
            {hint.icon}
          </span>
        )}
        {withMenu("fetch", "Auto-fetch settings", btn("fetch", noRemote, fetchTitle))}
        {withMenu(
          "pull",
          "More pull options",
          btn(
            "pull",
            noRemote || noBranch || !status?.upstream,
            !status?.upstream
              ? "No upstream branch"
              : status.ahead > 0 && status.behind > 0
                ? `Diverged from ${status.upstream} (↑${status.ahead} ↓${status.behind}): choose merge or rebase`
                : `Pull (fast-forward) from ${status.upstream}`,
            status?.behind ? `↓${status.behind}` : undefined,
            pull,
          ),
        )}
        {withMenu(
          "push",
          "More push options",
          btn(
            "push",
            noRemote || noBranch,
            status?.upstream ? `Push to ${status.upstream}` : "Publish this branch",
            status?.upstream ? (status.ahead ? `↑${status.ahead}` : undefined) : "new",
          ),
        )}
      </div>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={closeMenu}
          items={menu.kind === "fetch" ? fetchMenuItems : menu.kind === "pull" ? pullMenuItems : pushMenuItems}
        />
      )}
      {divergence && (
        <PullDialog
          divergence={divergence}
          onMerge={() => pullWith("merge")}
          onRebase={() => pullWith("rebase")}
          onCancel={() => setDivergence(null)}
        />
      )}
      {notice && (
        <div className={`notice ${notice.kind}`} onClick={() => setNotice(null)} title="Click to dismiss">
          <pre>{notice.text}</pre>
        </div>
      )}
    </>
  );
}
