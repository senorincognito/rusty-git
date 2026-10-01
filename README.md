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
- **Amend previous commit** switch above the message box: pre-fills the last commit's
  message and replaces that commit instead of creating a new one. Staged changes are folded
  in, and a message-only amend works with nothing staged. The author is kept and the
  committer becomes you, like `git commit --amend`. If the commit is already pushed, a
  warning says amending it needs a force push.
- Commit is refused when there is no message, nothing is staged, conflicts are
  unresolved, or `user.name` / `user.email` aren't configured. Errors are shown in the panel.
- Finishing an in-progress merge records the merge parents and clears the merge state.
- The graph, branch label and file lists reload automatically when the repo changes, even
  from outside the app (terminal, editor, other tools): the `.git` folder is watched for
  HEAD, index and ref changes. File lists also refresh when
  the window regains focus, so edits made in your editor show up.

### Commit details
- Click a commit in the graph: the right panel switches to that commit's message, author, date,
  parents and the files it changed (added, modified, deleted, renamed with their old path).
  Merge commits are shown against their first parent.
- Click a changed file: the centre area shows the file with the commit's changes marked in
  place (added lines in green with `+`, removed lines in red with `-`, old and new line
  numbers). A **Full file** switch toggles between the whole file and just the changed hunks
  with three lines of context. Binary and very large files are not previewed, and diffs are
  capped at 20,000 lines. **Back to graph** returns to the commit graph exactly where you left it.
- While you have uncommitted changes, a notice at the top of the panel says how many files
  changed in the working directory, with a **View changes** button that closes the commit view
  and returns to the staging panel (your draft commit message is kept). The × does the same.

### Commit graph context menu
- Right-click a commit: **Rename commit** opens an editor in the right panel with the full
  message and **Update** / **Cancel** buttons (Esc cancels, Ctrl+Enter updates). Only
  commits on the current branch can be renamed.
- Update rewrites the commit's message and rebuilds every later commit on the branch on top
  of it (same files, authors and dates, new ids), then moves the branch. Files and the index
  are untouched, so there can be no conflicts. Other branches keep their old history. The
  panel warns when later commits are rewritten and when the commit is already pushed (a
  force push is then needed).

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
- **Auto-fetch** (on by default, every 3 minutes): fetches in the background while the window
  is focused, and right away when you come back to a stale repo. It never overlaps another
  git operation, stays silent (a small spinner shows while it runs), backs off when the
  remote is unreachable, and pauses instead of retrying when credentials are needed. Switch it
  off or pick 1 / 3 / 5 / 10 minutes from the ▾ next to Fetch (or right-click Fetch).
- **Force push**: right-click the Push button, or click the small ▾ beside it. It asks for
  confirmation first (and says how many remote commits will be discarded), then runs
  `git push --force-with-lease`, which is refused if the remote moved since your last fetch.
  Use it after amending or renaming a commit that was already pushed.
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

Backend (`src-tauri/src/`), one module per concern, each with tests where it has logic:

| File | Purpose |
| --- | --- |
| `repo.rs` | Open repo, recent repos list |
| `graph.rs` | Commit walk, ref labels, lane layout |
| `changes.rs` | Status, stage / unstage, create commit |
| `branches.rs` | Local branches, create / checkout / delete |
| `remotes.rs` | Origin info, add origin, delete remote branch |
| `sync.rs` | Fetch / pull / push via system git, ahead/behind |
| `terminal.rs` | Pseudo-terminal sessions |
| `watch.rs` | Watches `.git` and emits `repo-changed` for live reload |

Frontend (`src/`). `@/` is an alias for `src/` (see `tsconfig.json` and `vite.config.ts`).

| Folder | Purpose |
| --- | --- |
| `api/` | Typed wrappers around the Tauri commands, one file per backend module |
| `features/welcome/` | Start screen: open a repo, recent repos |
| `features/repo/` | `RepoView`: the screen for an open repository (layout, live reload, shortcuts) |
| `features/graph/` | Virtualised commit graph |
| `features/changes/` | Staging and commit panel |
| `features/sidebar/` | Local branches, remotes, add origin |
| `features/toolbar/` | Title-bar buttons: fetch / pull / push, new branch |
| `features/terminal/` | xterm.js terminal panel |
| `components/` | Generic UI with no git knowledge: context menu, resizable panel, collapsible section |
| `hooks/` | Shared hooks, e.g. `useLatestRequest` (ignore out-of-order async responses) |
| `App.tsx` | Switches between the start screen and the repo view |
