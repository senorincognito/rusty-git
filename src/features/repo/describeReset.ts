import type { ResetInfo, ResetMode } from "@/api/history";
import { t } from "@/i18n";

/** The confirmation text for a reset: what moves and what happens to the staging area and the files. */
export function describeReset(info: ResetInfo, mode: ResetMode): string {
  const r = t.reset;
  const where = info.branch ? r.branch(info.branch) : r.detachedHead;
  const target = `${info.targetShort} "${info.targetSummary}"`;
  const lines: string[] = [
    info.sameCommit ? r.sameCommit(target, where) : r.move(where, info.fromShort, target),
  ];

  if (info.removed > 0) {
    lines.push("", r.removed(info.removed, where));
    for (const summary of info.removedSummaries) lines.push(`  • ${summary}`);
    if (info.removed > info.removedSummaries.length) {
      lines.push(r.more(info.removed - info.removedSummaries.length));
    }
  } else if (info.added > 0) {
    lines.push("", r.forward(where, info.added));
  }
  if (!info.isAncestor) {
    lines.push("", r.otherLine);
  }

  lines.push("");
  switch (mode) {
    case "soft":
      lines.push(info.removed > 0 ? r.softRemoved : r.soft);
      break;
    case "mixed":
      lines.push(r.mixed);
      break;
    case "hard": {
      let text = r.hard;
      if (info.removed > 0) text += r.hardRemoved;
      if (info.workingChanges > 0) text += r.hardLost(info.workingChanges);
      text += r.hardUntracked;
      lines.push(text);
      break;
    }
  }

  if (info.pushedRemoved > 0) {
    lines.push("", r.pushed(info.pushedRemoved));
  }
  if (info.removed > 0) {
    lines.push("", r.reflog);
  }
  return lines.join("\n");
}
