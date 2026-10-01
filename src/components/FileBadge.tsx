import { t } from "@/i18n";
import "./FileBadge.scss";

export type FileStatus =
  | "new"
  | "modified"
  | "deleted"
  | "typechange"
  | "conflicted"
  | "renamed"
  | "copied";

const LETTER: Record<FileStatus, string> = {
  new: "A",
  modified: "M",
  deleted: "D",
  typechange: "T",
  conflicted: "!",
  renamed: "R",
  copied: "C",
};

/** One-letter, colour-coded marker for a file's change status. */
export default function FileBadge({ kind }: { kind: FileStatus }) {
  return (
    <span className={`badge ${kind}`} title={t.fileStatus[kind]}>
      {LETTER[kind]}
    </span>
  );
}
