import { invoke } from "@tauri-apps/api/core";

export interface RefLabel {
  name: string;
  kind: "branch" | "remote" | "tag";
  isHead: boolean;
}

export interface Edge {
  col: number;
  color: number;
}

export interface GraphRow {
  id: string;
  shortId: string;
  summary: string;
  author: string;
  email: string;
  /** Unix seconds. */
  time: number;
  parents: string[];
  /** HEAD or an ancestor of it: can be rewritten on the current branch. */
  onHead: boolean;
  refs: RefLabel[];
  col: number;
  color: number;
  top: Edge[];
  through: Edge[];
  bottom: Edge[];
}

export interface Graph {
  rows: GraphRow[];
  maxLanes: number;
  hasMore: boolean;
}

export const getGraph = (path: string, limit: number) =>
  invoke<Graph>("get_graph", { path, limit });
