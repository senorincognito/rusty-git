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

const NAME: Record<FileStatus, string> = {
  new: "Added",
  modified: "Modified",
  deleted: "Deleted",
  typechange: "Type changed",
  conflicted: "Conflicted",
  renamed: "Renamed",
  copied: "Copied",
};

/** One-letter, colour-coded marker for a file's change status. */
export default function FileBadge({ kind }: { kind: FileStatus }) {
  return (
    <span className={`badge ${kind}`} title={NAME[kind]}>
      {LETTER[kind]}
    </span>
  );
}
