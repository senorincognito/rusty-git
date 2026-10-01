import type { ResetInfo, ResetMode } from "@/api/history";

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** The confirmation text for a reset: what moves and what happens to the staging area and the files. */
export function describeReset(info: ResetInfo, mode: ResetMode): string {
  const where = info.branch ? `branch "${info.branch}"` : "the detached HEAD";
  const target = `${info.targetShort} "${info.targetSummary}"`;
  const lines: string[] = [
    info.sameCommit
      ? `Reset to ${target}, the commit ${where} is already on. No commits move.`
      : `Move ${where} from ${info.fromShort} to ${target}.`,
  ];

  if (info.removed > 0) {
    lines.push("", `${plural(info.removed, "commit")} will no longer be on ${where}:`);
    for (const summary of info.removedSummaries) lines.push(`  • ${summary}`);
    if (info.removed > info.removedSummaries.length) {
      lines.push(`  … and ${info.removed - info.removedSummaries.length} more`);
    }
  } else if (info.added > 0) {
    lines.push("", `${where} moves forward by ${plural(info.added, "commit")}.`);
  }
  if (!info.isAncestor) {
    lines.push("", "This commit is not part of the current history, so the branch jumps to a different line of history.");
  }

  lines.push("");
  switch (mode) {
    case "soft":
      lines.push(
        info.removed > 0
          ? "Soft reset: the staging area and your files are not touched. The changes of the removed commits show up as staged changes, ready to be committed again."
          : "Soft reset: the staging area and your files are not touched.",
      );
      break;
    case "mixed":
      lines.push(
        "Mixed reset: the staging area is reset to that commit; your files are not touched. Everything that was staged, " +
          "and the changes of the removed commits, becomes unstaged changes (new files become untracked).",
      );
      break;
    case "hard": {
      let text = "Hard reset: the staging area and your files are reset to that commit.";
      if (info.removed > 0) text += " The changes of the removed commits disappear from your files.";
      if (info.workingChanges > 0) {
        text += ` Your ${plural(info.workingChanges, "uncommitted file change")} will be lost for good (git cannot recover them).`;
      }
      text += " Untracked files are left alone.";
      lines.push(text);
      break;
    }
  }

  if (info.pushedRemoved > 0) {
    lines.push(
      "",
      `${info.pushedRemoved} of the removed commits ${info.pushedRemoved === 1 ? "is" : "are"} already pushed: ` +
        "publishing the new position will need a force push.",
    );
  }
  if (info.removed > 0) {
    lines.push("", "The removed commits stay in git's reflog for a while, so they can be recovered with git reflog.");
  }
  return lines.join("\n");
}
