# CLAUDE.md

Guidance for working on **Rusty Git Client**: a desktop Git GUI in the spirit of GitKraken, built with
Tauri 2 (Rust backend, `git2`/libgit2 plus the system `git`) and React + TypeScript + Vite.
Targets Windows and macOS. `README.md` lists user-facing features; this file is for contributors.

**Keep this file current.** The author has given standing permission to update it without asking: when work
adds a module or a rule, settles a product decision, or uncovers a quirk (library, git, Windows, tooling),
record it here in the right section, briefly and with the *why*. Edit existing entries rather than
duplicating them, and move finished items off "Not implemented yet".

## Commands

```sh
npm install
npm run tauri dev                                    # run the app (hot reload; a Rust change restarts it)
npx tsc --noEmit                                     # typecheck the frontend
npx vite build                                       # bundle (also proves the "@/" alias resolves)
cargo test --lib --manifest-path src-tauri/Cargo.toml   # all backend tests (needs system `git` on PATH)
cargo check --manifest-path src-tauri/Cargo.toml     # keep it warning-free
```

Before calling work done: `cargo test --lib`, `npx tsc --noEmit`, `npx vite build`, and a CSS brace
balance check (see Pitfalls). There is no frontend test runner and the UI can't be driven from here, so
say plainly that UI behaviour is untested in the running app.

## Architecture

**Data flow.** The frontend calls typed wrappers in `src/api/*`, which `invoke()` Tauri commands in
`src-tauri/src/*`. `watch.rs` watches `.git` (HEAD, index, `refs/`) and emits `repo-changed`;
`RepoView` listens, bumps `graphKey`, and every panel reloads from it. So any change made anywhere
(terminal, editor, another tool) shows up without a manual refresh.

### Backend (`src-tauri/src/`)

| Module | Commands / role |
| --- | --- |
| `repo.rs` | `open_repo`, recent repos (JSON in the app data dir) |
| `graph.rs` | `get_graph`: revwalk over all refs, lane layout computed in Rust, `on_head` flag per row |
| `changes.rs` | status, stage/unstage, `create_commit` (new or `amend`), `get_head_commit` |
| `commit_detail.rs` | all diff rendering: `get_commit_detail` (files of a commit, renames), `get_file_diff` (a commit's file), `get_working_diff` (staged/unstaged file); shared `diff_options` + `render_diff` |
| `history.rs` | `get_rename_info`, `rename_commit_message` (rewrites the commit and its descendants); `is_pushed` |
| `branches.rs` | list, create+checkout, checkout, delete, rename (local) |
| `remotes.rs` | `origin` info, add origin, delete and rename remote branches |
| `sync.rs` | fetch / pull / push / force push / auto-fetch / diverged pull; `run_git`, `run_git_with` |
| `terminal.rs` | PTY sessions (`portable-pty`) feeding the xterm.js panel |
| `watch.rs` | the `.git` watcher |

### Frontend (`src/`, `@/` = `src/`)

```
App.tsx                    Welcome or RepoView
api/                       one typed wrapper file per backend module
features/welcome|repo|graph|changes|commit|rename|sidebar|toolbar|terminal/
components/                generic UI: ContextMenu, Modal, ResizablePanel, Section, FileBadge
hooks/                     useLatestRequest, usePersistentState, useAutoFetch
App.css                    the single stylesheet (CSS variables for the dark theme)
```

`RepoView` owns the screen state: `graphKey`, `terminalOpen`, `selectedCommit`, `openFile` (a commit's file),
`openWorkingFile` (a staged/unstaged file from the Changes panel), `renaming`.

- **Right panel** (a `ResizablePanel`) shows one of: `RenameCommit` > `CommitDetail` > `Changes`.
  `Changes` stays mounted but hidden so a half-typed commit message survives.
- **Centre** shows the graph, or `FileDiff` on top of it. `FileDiff` takes a `source`: a commit (opened from
  `CommitDetail`), or `staged`/`unstaged` (opened from the Changes lists). Working-tree sources refresh on
  `graphKey` and window focus (file edits aren't watched) without resetting scroll. The graph stays mounted (hidden via
  `position: absolute; visibility: hidden`, NOT `display: none`) to keep its scroll position.
- Graph selection is controlled by `RepoView` (`selectedId`/`onSelectCommit`); right-click does not select.

### Persisted state

`recent_repos.json` in the app data dir; `localStorage` keys `sidebarWidth`, `changesWidth`,
`autoFetch.enabled` (default true), `autoFetch.seconds` (default 180), `diff.fullFile` (default true).
All of it is keyed by the bundle identifier `com.gitclient.app`, which is **deliberately unchanged** by the
app rename so users keep their data. Don't change it casually.

## Conventions

- **Commands** are `async` and run blocking work in `tauri::async_runtime::spawn_blocking` (see the
  `blocking` helpers). Register each in `lib.rs`. Serde structs use `rename_all = "camelCase"`; Tauri maps
  JS `oldPath` to Rust `old_path` automatically.
- **git2 vs system git.** Local reads/writes use `git2`. Anything that talks to a remote or needs
  credentials, merges or rebases uses the system `git` via `run_git` / `run_git_with` (user's credential
  helper, SSH agent, config apply; `GIT_TERMINAL_PROMPT=0`, no console window on Windows).
- **Async UI requests** must ignore out-of-order responses: use `useLatestRequest`
  (`const isCurrent = start(); ...; if (isCurrent()) set(...)`).
- **Popovers/menus** use `ContextMenu` (portal to `<body>`; supports `checked`, `separatorBefore`,
  `danger`, `disabled`+`title`). Split buttons (Fetch/Pull/Push) use `withMenu` in `SyncBar`: arrow click or
  right-click opens it; the arrow's `onMouseDown` stops propagation so the outside-click handler doesn't
  fight the toggle. Dialogs use `Modal`; simple yes/no uses `confirmDialog` (native, Tauri dialog plugin).
- **Destructive actions** confirm first, say what is lost, and prefer safe variants
  (`--force-with-lease`, safe checkout, abort on conflict). Disabled menu items carry a tooltip saying why.
- **Tests** build real repos in temp dirs (git2 and, for network behaviour, the system git with local bare
  remotes). Add a test with every backend feature. Commit messages end with the attribution trailer given in
  the session context.
- **Keyboard**: a global (window-level) `Escape` handler must ignore events from editable targets
  (`input`/`textarea`/`select`/contenteditable; xterm's hidden textarea counts) and must do nothing while a
  `.ctxmenu` or `.modal-backdrop` is open, because those handle their own Escape. `FileDiff` is the model.
  Existing shortcuts: Esc (close diff / menus / dialogs / editors), Ctrl+\` terminal (`RepoView`),
  Ctrl/Cmd+Enter commit and rename-update, arrow keys on resize handles. Keep the README table in sync.
- Match the surrounding comment density: short comments that explain *why*, none restating the code.

## Product decisions already made (keep consistent)

- **Pull** fast-forwards only (`--ff-only`). If the branches diverged, a dialog lists both sides and offers
  **Merge (default, preselected)**, Rebase, Cancel. The Pull button's arrow/right-click menu has
  "Pull (merge)" and "Pull (rebase)" to skip the dialog. All run with `--autostash`. On **conflicts the
  operation is aborted automatically** (conflicting files named, repo left untouched) since there is no
  conflict UI yet. `pull.rebase` is intentionally not consulted.
- **Force push** is `git push --force-with-lease`, behind a confirmation that counts the remote commits
  that will be discarded. Only offered when the branch has an upstream.
- **Auto-fetch**: on by default, every 180 s, only while the window is focused/visible, never overlapping a
  manual git operation, exponential backoff when offline, pauses (does not retry) on auth errors, resumes
  after a successful manual fetch/pull/push. Uses `--no-write-fetch-head --no-auto-gc`, 90 s timeout.
- **Amend** switch pre-fills the last message, keeps the author, makes you the committer, works with
  nothing staged, warns when the commit is already pushed.
- **Rename commit** (graph context menu) only for commits on the current branch. Rebuilds the commit and
  every later commit with identical trees/authors/dates, then moves the branch; other branches keep the old
  history. Warns about rewritten descendants and pushed commits.
- **Branch rename** (inline editor in the sidebar) is not allowed for the checked-out branch (local) or the
  branch the checked-out branch tracks (remote). Remote rename = one atomic push of the new name plus
  deletion of the old one, guarded by a lease; local branches that tracked it are repointed.
- **Working-tree diffs** (click a file in Changes): *unstaged* = index vs file on disk (untracked files show as
  all additions), *staged* = HEAD vs index (on an unborn branch everything staged is an addition). Staging
  buttons on a row stop propagation so they don't also open the diff. Committing closes an open working diff.
- Commit detail diffs are against the **first parent**; renames detected; 2000-file and 20 000-line caps;
  binary/over-5 MB files are not previewed. The diff view has a "Full file" switch (default on).

## Pitfalls learned the hard way

**libgit2 / git2 0.21**
- Many accessors return `Result`, not `Option`: `Commit::message()`, `summary()` (`Result<Option<&str>>`),
  `Signature::name()/email()`, `Reference::name()/shorthand()`. `StringArray::iter()` yields
  `Result<Option<&str>>` (flatten twice). `DiffOptions::max_size` takes `i64`. `mergehead_foreach` needs
  `&mut Repository`, so call it before creating a `Tree` that borrows the repo.
- **`Branch::rename` deletes the old ref before failing on a directory/file clash** (`feature` vs
  `feature/x`): the branch is lost. `rename_branch` pre-checks clashes and restores the branch on failure.
- `repo.commit(Some("refs/heads/x"), ...)` requires the first parent to be the ref's current tip (matters
  when building test histories).
- Use `disable_pathspec_match(true)` when a diff is limited by a literal file path.
- `diff_index_to_workdir` only emits lines for untracked files with `include_untracked(true)` **and**
  `show_untracked_content(true)`; `diff_tree_to_index` takes `None` for the tree on an unborn branch.

**git CLI**
- A rejected `--force-with-lease` push with `--atomic` prints "atomic push failed"; only fall back to a
  non-atomic push on "does not support --atomic", or a half-done rename results.
- Killing `git` leaves helpers (`git-remote-https`, ssh) holding the stdout/stderr pipes, so reading them
  blocks. `run_git_with` kills the whole process tree (`taskkill /T` on Windows, a process group on unix)
  and does not wait on the pipes after a timeout.
- Fetch rewrites `.git/FETCH_HEAD`, which the watcher sees; silent fetches pass `--no-write-fetch-head`
  (git >= 2.29, with a fallback for older git).

**Windows**
- ConPTY asks the terminal for the cursor position (`ESC[6n`) and waits for the answer. xterm.js replies
  automatically; terminal tests must reply `ESC[1;1R` themselves.
- `canonicalize()` yields `\\?\` paths; strip the prefix before showing/storing them.
- Spawned git processes use `CREATE_NO_WINDOW`. Git config has `core.autocrlf=input`; CRLF warnings on
  commit are harmless.

**CSS**
- A merge once dropped a closing `}` in `App.css`; everything after it silently nested and stopped
  applying (menus rendered unstyled). Check balance after CSS edits:
  `node -e 'const s=require("fs").readFileSync("src/App.css","utf8");let d=0;for(const c of s){if(c==="{")d++;if(c==="}")d--;}console.log(d)'`
  must print `0`.

## Working in this environment (Claude Code on the author's Windows machine)

- The Bash tool is Git Bash. A shell opened before a tool was installed may lack it: prefix
  `export PATH="$PATH:$HOME/.cargo/bin"` for cargo. If the user's own terminal says `cargo metadata ... program
  not found`, they need to restart VS Code so PATH refreshes.
- **Python is not installed.** Use Node for scripted edits.
- **Write scripts with the Write tool, not shell heredocs.** The shell layer mangled sequences such as a
  backslash followed by a backtick (a `sed` clean-up put a stray backtick at the start of every line), and
  complex quoting failed. Put scratch scripts in the session scratchpad, run them with `node "<windows path>"`,
  and verify results with `Read`/`grep` rather than assuming. Build escape characters from
  `String.fromCharCode` if a script must write them.
- **`node` resolves `/tmp` as `D:\tmp`**, unlike Git Bash. Pass real Windows paths (`cygpath -w`) to node.
- A reliable scripted-edit pattern: read the file, normalise `\r\n` to `\n`, `must(text.includes(anchor))`,
  replace, write back with the original EOL. `tsconfig.json`, `vite.config.ts`, `tauri.conf.json`, `index.html`
  and `main.rs` use CRLF; most other files are LF.
- **Splitting commits by feature when features share files**: stage the feature-only files with `git add`, and
  for shared files build the intended content (HEAD plus that feature's edit, or the working copy minus the
  other feature's block), then
  `git update-index --cacheinfo 100644,$(git hash-object -w --stdin),<path>`. Inspect with
  `git diff --cached --stat` before committing.
- The Vite dev server on port 1420 keeps running; a Rust change restarts the app, which returns to the
  welcome screen (open a repo again to see repo-view changes).

## Git workflow with this user

- Commit only when asked; **never push unless asked** (the user often pushes themselves). Branch is `main`,
  remote `origin` (`github.com/senorincognito/rusty-git`).
- The user sometimes commits between turns and switches branches (using this very app). Re-check
  `git status`/`git log` before committing; files can differ from what you last saw.
- End commit messages with the attribution trailer from the session context.
- They like concise "what changed / what's not done" summaries, honest notes about what was not verified in
  the running UI, and a question about commit grouping at the end. They sometimes write in German.

## Not implemented yet

Tags and stashes in the sidebar; hunk/line staging from the diff view; checkout of remote branches; merge/rebase as standalone actions; discard
changes and stash; conflict resolution UI (pulls with conflicts are aborted); multiple remotes (only
`origin`); syntax highlighting and intra-line diff highlighting; side-by-side diff; a
"you rewrote pushed history, force push instead" hint in the diverged-pull dialog; a conflict preview
(`git merge-tree`) before pulling; keyboard navigation in the graph.
