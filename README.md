# Git Client

A simple desktop Git GUI inspired by GitKraken's interface. Built with
[Tauri 2](https://tauri.app) (Rust backend using [libgit2](https://libgit2.org) via `git2`)
and React + TypeScript + Vite. Targets Windows and macOS.

## Features

### Repositories
- Open any folder through the native picker. The repo is found by searching upward, so
  picking a subfolder works.
- Recent repositories list (up to 20, newest first), stored in the app data directory.
  Entries whose folder no longer exists are dropped; individual entries can be removed.
- Shows the current branch, or `detached @ <sha>` for a detached HEAD, and handles
  freshly initialised repos with no commits.

### Commit graph
- All local branches, remote branches and tags, plus a detached HEAD, newest first.
  Stash and notes refs are left out.
- Coloured lane layout computed in Rust: branches, merges and joins are drawn as curved
  lines per row.
- Ref chips on each commit (current branch highlighted; branches, remotes and tags
  styled separately).
- Columns for message, author, date and short hash.
- Virtualised rendering, with history loaded in pages of 1000 as you scroll.
- Click a row to select it.

### Creating commits
- **Changes panel** with Unstaged and Staged file lists and status badges
  (A added, M modified, D deleted, T type change, ! conflicted).
- Stage or unstage individual files, or all at once. Deleted files and repos without
  any commits yet are supported.
- Commit message box. Commit with the button or `Ctrl`/`Cmd` + `Enter`.
- Commit is refused when there is no message, nothing is staged, conflicts are
  unresolved, or `user.name` / `user.email` aren't configured. Errors are shown in the panel.
- Finishing an in-progress merge records the merge parents and clears the merge state.
- The graph and branch label refresh after each commit, and the file lists refresh when
  the window regains focus, so edits made in your editor show up.

## Not yet implemented

- Diff view and commit detail panel
- Sidebar for branches, remotes, tags and stashes
- Branch create / checkout / merge / rebase
- Fetch, pull and push, and credential handling
- Amend, discard changes, stash
- Renames are shown as a delete plus an add

## Development

Requirements: Node.js, Rust (stable, MSVC toolchain on Windows), and on Windows the
Visual Studio C++ Build Tools. WebView2 ships with Windows 11.

```sh
npm install
npm run tauri dev      # run the app with hot reload
npm run build          # typecheck and build the frontend
cargo test --manifest-path src-tauri/Cargo.toml   # backend tests
```

## Layout

| Path | Purpose |
| --- | --- |
| `src-tauri/src/repo.rs` | Open repo, recent repos list |
| `src-tauri/src/graph.rs` | Commit walk, ref labels, lane layout (with tests) |
| `src-tauri/src/changes.rs` | Status, stage / unstage, create commit (with tests) |
| `src/git.ts` | Typed wrappers around the Tauri commands |
| `src/Graph.tsx` | Virtualised commit graph |
| `src/Changes.tsx` | Staging and commit panel |
| `src/App.tsx` | Welcome screen and repo view |
