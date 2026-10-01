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
- The graph, branch label and file lists reload automatically when the repo changes, even
  from outside the app (terminal, editor, other tools): the `.git` folder is watched for
  HEAD, index and ref changes. File lists also refresh when
  the window regains focus, so edits made in your editor show up.

### Branches and remotes (left panel)
- **Local branches**: alphabetical list with the current branch highlighted and `↑n` / `↓n`
  when ahead of or behind the upstream. Double-click a branch to check it out (safe
  checkout: refused if uncommitted changes would be overwritten).
- **Branch** button: create a branch from the current commit and check it out; the name is
  validated and uncommitted changes carry over.
- **Remotes**: shows `origin` with its URL and remote branches. When there is no origin yet,
  a form adds one.
- **Context menus** on branches: *Delete branch* (disabled for the checked-out branch) and
  *Delete remote branch* (disabled for the branch the checked-out branch tracks). Both
  confirm first and warn when commits would exist nowhere else. Remote deletion runs
  `git push origin --delete`.

### Fetch, pull and push
- Title-bar buttons run the system `git`, so your credential helper and SSH setup apply.
- Fetch updates all remotes (with prune). Pull is fast-forward only, so it never creates a
  surprise merge. Push publishes a new branch to `origin` and sets its upstream.
- Buttons show `↓n` / `↑n` counts and are disabled with a tooltip when they can't work.

### Terminal
- Toggle a real terminal (PowerShell on Windows, your login shell on macOS) in the
  repository folder with the **>_ Terminal** button or the Ctrl + Backquote shortcut. Resizable, keeps its
  session while hidden, restarts when you switch repositories.
- Anything you run there (commits, checkouts, ...) shows up in the UI through live reload.

## Not yet implemented

- Diff view and commit detail panel
- Tags and stashes in the sidebar, remote branch checkout
- Merge / rebase, rename branch, push or pull from the context menu
- Multiple remotes (only `origin` is handled)
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
| `src-tauri/src/watch.rs` | Watches `.git` and emits `repo-changed` for live reload |
| `src-tauri/src/branches.rs` | Local branches, create / checkout / delete (with tests) |
| `src-tauri/src/remotes.rs` | Origin info, add origin, delete remote branch (with tests) |
| `src-tauri/src/sync.rs` | Fetch / pull / push via system git, ahead/behind (with tests) |
| `src-tauri/src/terminal.rs` | Pseudo-terminal sessions (with tests) |
| `src/git.ts` | Typed wrappers around the Tauri commands |
| `src/Graph.tsx` | Virtualised commit graph |
| `src/Changes.tsx` | Staging and commit panel |
| `src/Sidebar.tsx` | Local branches and remotes panel |
| `src/SyncBar.tsx`, `src/BranchButton.tsx` | Title-bar buttons |
| `src/ContextMenu.tsx` | Reusable context menu (portal popover) |
| `src/TerminalPanel.tsx` | xterm.js terminal panel |
| `src/App.tsx` | Welcome screen and repo view |
