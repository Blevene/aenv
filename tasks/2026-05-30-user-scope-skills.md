# User-scope skills (global-profile skill import) — v0.3.0

**Problem:** `[[skills]]` entries can't reach a global profile. `SkillDecl.scope` exists in the schema (Issue #4 Task 4) but `resolve.rs` hardcodes every skill candidate to `Scope::Project` (resolve.rs:601, :640) and always uses the project `skills_dir`. Global activation only materializes `User` candidates, so skills declared in a global namespace silently never appear. And `aenv skill import` has no `--scope` flag.

**Goal:** `aenv skill import <src> --ns <ns> --scope user [--path …] [--pin …]` adds a skill that materializes under the adapter's `user_skills_dir` (`~/.claude/skills/<name>/`) on `aenv global use <ns>`. Same for `aenv skill new --scope user`.

## Changes

### 1. Core resolver (`crates/aenv-core/src/resolve.rs`)
In the skill-candidate gathering (~lines 538–645):
- `let skill_scope = decl.scope;`
- Select skills dir by scope: `Project → adapter.skills_dir`; `User → adapter.user_skills_dir` (strip a leading `~/`). Error if the chosen one is absent.
- `dest_prefix = "{skills_dir}/{decl.name}"` (same relative form for both; scope picks the root).
- **Authored:** walk base = `scope==User ? ns_root.join("user") : ns_root`; `source_path = base.join(rel_str)`.
- **Imported:** unchanged source (cache); only dest_prefix + scope change.
- Both `out.push(Candidate{…})` sites: `scope: skill_scope` (not hardcoded Project).
- The existing `validate_candidate_paths` `~/`-rejection still holds (dest is tilde-stripped).

### 2. CLI (`crates/aenv-cli/src/main.rs` + `cmd/skill/{import,new}.rs`)
- Add `--scope <project|user>` (default project) to `SkillImport` and `SkillNew`.
- Thread to the written `SkillDecl.scope`. For `skill new --scope user`, scaffold the SKILL.md under `<ns>/user/<user_skills_dir>/<name>/`.

### 3. Tests
- Core unit (`tests/`): resolver tags a `scope="user"` authored skill as User with dest `.claude/skills/<name>/…`.
- CLI e2e: `skill import … --scope user` into a namespace, `global use`, assert `~/.claude/skills/<name>/SKILL.md` materializes.

### 4. Docs
- `skills/aenv/SKILL.md`: document global/user-scope skills + `--scope user`.
- README skill row + CHANGELOG.

## Then
5. Import the requested skills into `research` at user scope:
   - K-Dense (ref main, `--path skills/<name>`): exploratory-data-analysis, autoskill, get-available-resources, infographics, networkx, markitdown, peer-review, latex-posters, matplotlib, scientific-brainstorming, scientific-critical-thinking, scientific-writing, scikit-learn, seaborn, pytorch-lightning, statistical-analysis, statsmodels, torch-geometric, transformers, what-if-oracle.
   - microsoft/ai-agents-for-beginners (ref main, `--path .agents/skills/jupyter-notebook`).
   - NeoLabHQ/context-engineering-kit (ref master): descend `plugins/reflexion/skills/*`, `plugins/review/skills/*`.
   - mlflow/skills (ref HEAD): each skill dir (agent-evaluation, analyze-mlflow-chat-session, analyze-mlflow-trace, instrumenting-with-mlflow-tracing, mlflow-agent, mlflow-onboarding, querying-mlflow-metrics, retrieving-mlflow-traces, searching-mlflow-docs).
6. Release v0.3.0 (bump, CHANGELOG, pre-tag ritual, tag, verify), reinstall locally.

## Gates (per tasks/lessons.md): fmt, clippy -D warnings, test, `RUSTDOCFLAGS=-D warnings cargo doc`.
