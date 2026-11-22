# Agent notes

Biblio is a native Rust application built with NSH. It uses resource schemas to manage different kinds of files in one library. Each schema defines accepted file types, metadata fields and import steps.

## Repo

### Commands

| Purpose | Command |
| --- | --- |
| Run the native app | `cargo run --locked` |
| Check before commit (release CI) | `cargo test --locked --workspace` |
| Test an existing library upgrade | `cargo test --locked -p biblio-core --lib database::tests::opens_populated_tauri_database_and_preserves_migration_history -- --exact` |
| Check all targets | `cargo check --locked --workspace --all-targets` |
| Build a local macOS bundle | `scripts/bundle-macos.sh release` |

### Layout

- `src/main.rs` selects the profile and starts the native platform host. Both the launcher and the headless tests in `crates/biblio-ui/tests/` open the window through `biblio_ui::root`.
- `crates/biblio-ui/src/` owns NSH windows, views and controls. `crates/biblio-core/src/` owns the catalog, migrations, import pipelines, remote providers and transfer workers.
- `assets/` holds source artwork. `packaging/` holds the platform resources used to build the app bundle.

### Gotchas

- The default macOS profile is the installed app’s profile, including `~/Library/Application Support/io.augite.biblio/biblio.db`. Set `BIBLIO_CONFIG_DIR` to use an isolated development profile. Reserve resets for disposable development data.
- Imports copy or move files into the configured storage folder under their category. Recheck the storage path when a settings dialog closes.
- Uploads go through `upload_worker::wrap_and_upload`, which encrypts each file with `services::container` before sending it. Remote names are opaque and extensionless. The catalog holds the original filenames. The encryption key lives in `app_settings`. Losing the key makes encrypted files unrecoverable.
- Dispatch downloads using `files.remote_container` alone. `bbx1` identifies a single encrypted object even above today’s size threshold, `bbx1-split` identifies ordered encrypted parts, and SQL `NULL` identifies a legacy raw object. Use `reencrypt_worker` for legacy backfills.
- Uploads above 10 GiB of plaintext split into independent encrypted objects. Their identifiers live in `remote_parts`. The parent row has a placeholder path and null `remote_fs_id` and `remote_md5`. `download_worker::download_split` appends decrypted parts in order and checks the result against `remote_size`.
- Delete remote files through `remote::delete_on_remote_for_file` so every remote part is removed. The foreign-key cascade removes the corresponding local part rows.
- Baidu slice uploads assume SVIP (32 MiB per slice). Other tiers support 16 or 4 MiB, and a block list is limited to about 2,048 slices. Keep `SLICE_CONCURRENCY = 1` because concurrent slice requests can lose their bodies. Use the 600-second `SLICE_TIMEOUT` for slice requests.

## Principles

### Project status

- **Status:** `release`
- **Breaking changes:** Allowed for long-term better design, but also prefer the smallest compatible change when it does not trap the design.
- **Persist data:** Must migrate. Do not reset.

| Status | Breaking changes | Persist data |
| --- | --- | --- |
| `startup` | Allowed whenever they make the long-term design better. Do not optimize for compatibility or the smallest diff. | May reset. No migration. |
| `release-solo` | Same as `startup`. | Must migrate. Do not reset. |
| `release` | Allowed for long-term better design, but also prefer the smallest compatible change when it does not trap the design. | Must migrate. Do not reset. |

### Docs

`docs/` holds the current state of the work, not a history of changes. One subject is one file: `docs/<category>/<slug>.md`, with `slug` lowercase and hyphenated. The one file outside a category is `docs/GLOSSARY.md`. The one history is `docs/spec/archived/`.

| Category | Holds | Updated when |
| --- | --- | --- |
| `polaris` | One long-range goal: what will be true when it is reached, the signal that shows it, the route and its parts, and the features cut from them | The user sets, reviews, or reroutes the goal, a feature on its list is decided, or the goal is reached. A dropped goal is deleted |
| `feature` | One feature: what a user can do, its limits, what it is not, and the decisions that shaped it with the option each rejected | The user confirms a new feature or a change to one, and when the code ships, changes, or removes it |
| `spec` | How the code will deliver one feature, under the same slug as its feature doc: the owner of each behavior, the interfaces of the modules it adds or changes, seams, stored data and its migration, and the steps to land it. Open only while that work is unfinished | Its feature changes, or the work finishes and the spec moves to `docs/spec/archived/` |

- `docs/GLOSSARY.md` is the project's one glossary: one term per concept, with the words to avoid. One word never names two things.
- A goal says `Status: active` while it is pursued, and `Status: reached` once its signal holds. A feature doc says `Status: decided` while it is ahead of the code, and `Status: shipped` once the code does what it says.
- Three kinds of doc describe what is not true yet: an active goal says where the project is going, a `decided` feature says what it will do, and an open spec says how the code will do it. An open spec, `docs/spec/<slug>.md`, means its feature is not done. A spec holds nothing that must outlive the work: decisions go in the feature doc, terms in the glossary.
- Work a spec step by step, in the order its `After` lines allow, and do not edit it while working it; only `to-implement-spec` changes a spec. Before work starts or resumes, check that the feature-doc hash in its header still matches `git hash-object docs/feature/<slug>.md`; when it does not, stop, because the spec must catch up with the feature first. Each step is one commit whose message ends with the trailer `Spec-Step: <slug>#<n>`. The commit that lands the last step sets the feature doc to `shipped` and moves the spec to `docs/spec/archived/<YYYY-MM-DD>-<slug>.md` with `git mv`, dated that day.
- `docs/spec/archived/` is a record, not a source. Nothing reads, searches, or updates it, except a review of the branch that archived a spec, or a user who asks. Its `.ignore` keeps ripgrep out. The feature doc and the code say what is true now.
- Find the existing file before writing: `ls docs/*/` and `rg -il '<subject>' docs`. Update that file. If more than one file covers the subject, ask which one stays. Create a file only when none exists.
- State what is true now: what it is and what it is not. Replace statements that stopped being true. Do not append how it changed, and do not write a plan (goals, steps, or future work) outside a goal or a spec.
- A code change that makes a doc untrue updates the doc in the same commit. A removed feature removes its doc.
- Outside `docs/spec/archived/`, no dated files, changelogs, or second files for one subject. History lives in git.
- The current state must be allowed by Project status for breaking changes and persisted data.
- Rules for agents live in `AGENTS.md`, not in `docs/`.

### Commits

Commit locally as the work goes, one verified step at a time. Do not leave a task as one large uncommitted change, and do not end it with one commit that mixes everything.

- At the start of a task, note `git rev-parse HEAD`. The whole task can then be read as `git log --oneline <start>..HEAD` and reviewed as `git diff <start>..HEAD`.
- A step is one logical change that leaves the tree building and its tests passing. A refactor, a rename, or a formatting pass is its own commit, separate from a behavior change. A test that pins a bug may land with its fix.
- Before each commit, run the check for the surface you touched. Do not commit a red step. When a hook fails, fix the cause; never skip it with `--no-verify`.
- Stage only the files of this step: `git add <paths>`, then `git diff --cached --stat`. Never use `git add -A`, `git add .`, or `git commit -a` while the tree holds work you did not do.
- Subject: imperative, under 72 characters, says what changed. Body: why, and how it was verified when that is not obvious. A bug fix names the root cause.
- A change that makes a doc untrue updates that doc in the same commit.
- Never commit secrets, scratch files, debug lines, or build output.
- Commit on the current branch. Amending your own unpushed commit from this task is fine. Do not create or switch branches, rebase, or rewrite pushed history unless asked.
- Commits stay local. Push, tag, publish, or open a pull request only when asked.

### Cleanup

Intermediate artifacts are not part of the work. Create them where they cannot be committed, and remove them before the task ends.

- Scratch goes outside the repo, under `$TMPDIR` (else `/tmp`): throwaway scripts, harnesses, probes, captured logs, screenshots, reports, and temporary worktrees. When a tool must write inside the repo, use `.scratch/`, which is gitignored.
- Tag each temporary debug line with `[DEBUG-` and a short hex id, such as `[DEBUG-a4f2]`, so one search finds them all. A diagnostic worth keeping goes behind the project's debug flag instead.
- Before each commit and before the task ends, check:

  ```bash
  git status --short -uall                             # only intended files are changed or new
  rg -n '\[DEBUG-[0-9a-f]+\]' . --glob '!AGENTS.md'   # no temporary debug lines remain
  git worktree list                                    # remove the ones you added: git worktree remove --force <path>
  ```

- Remove only what you created in this task. Never delete or move files you did not create: untracked files that were there before you started, caches the project relies on (`node_modules`, `target`, `.venv`), or user data.
- A generated file the repo tracks is not scratch. Regenerate it with the project's command and commit it with the change that caused it.
- When cleanup cannot finish (a file is locked, a process is still running), say what is left and where.

### Architecture

Design and change code so that the changes this repo actually makes stay local, and a failure leaves consistent state. Judge structure by those two outcomes, not by resemblance to a named style.

- **Deep modules.** A module puts a lot of behavior behind a small interface. Before adding a module, ask whether deleting it would spread complexity back into its callers. If not, do not add it. Prefer fewer entry points and simpler parameters, with more decided inside.
- **Shared code does not know its callers.** A shared module never imports from a caller, and never branches on a caller's name, type, or config key. Features compose shared modules. The core does not implement a feature.
- **Extract only what is already shared.** Lift behavior out when two callers already need it and change it together. Copies that evolve apart stay separate. Add no abstraction, option, or hook for a need that does not exist yet.
- **Dependencies point inward.** Core logic does not import UI, frameworks, or concrete storage and network clients. No import cycles between modules.
- **Seams where something varies.** Pass a dependency in at a seam only when it has two adapters; production and test count as two. Tests go through a module's interface, not its internals.
- **One writer per fact.** Each piece of state has one authoritative write point. A step that can fail leaves state consistent: transactions, idempotent retries, and write-to-temp-then-rename for files.
- **Code that changes together lives together.** When one kind of change keeps touching the same set of modules, move the boundary instead of patching each side again.
- **One name per concept.** Use the terms in `docs/GLOSSARY.md`. One word never names two things.

How far a change may break existing interfaces and data follows Project status.

### Project rules

- Preserve the checksums of applied SQLite migrations, including the baseline in `schema.sql`. Add the next numbered SQL entry to `get_migrations()` in `crates/biblio-core/src/database/mod.rs`. SQLx applies it when the library opens.
- Understand the problem before changing code. Explore, plan, execute, then validate.
- Ask the user when a requirement is unclear or ambiguous.
