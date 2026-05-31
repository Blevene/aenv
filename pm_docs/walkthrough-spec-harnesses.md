# Walkthrough: building the three spec example harnesses

**Tested against:** `main`, `aenv 0.3.0`.
**Goal:** instantiate the four namespaces from functional spec §2 + §4 as a
working registry — `base`, `experiments`, `detailed-execution`, `analyst` —
including authored skills, imported skills, parameter overrides, and policies.

## How this differs from `walkthrough-three-harnesses.md`

The existing walkthrough uses three simplified namespaces with one-paragraph
CLAUDE.md files and no skills. Its purpose is the §7.5 scripted-comparison and
scriptability smoke test — showing hash capture, drift detection, and
enforce-policy rollback against the simplest possible manifests.

This walkthrough is the spec-verbatim instantiation. It follows §2.1–§2.3 and
§4.1–§4.4 closely, writes SKILL.md bodies that the spec described in prose but
didn't fully spell out, and verifies the structural differences the spec asserts
(`aenv diff` between pairs is the payoff). If you want to understand what the
spec harnesses actually look like on disk, this is the one to follow.

---

## Prerequisites

Build the release binary:

```bash
cargo build --release
```

Set up a scratch `AENV_HOME` (so this walkthrough doesn't touch your real
`~/.aenv/`) and a scratch project directory:

```bash
export AENV_HOME=$(mktemp -d -t aenv-spec-walk-XXXXXX)
export PROJECT=/tmp/aenv-spec-walk-proj
export BIN=$PWD/target/release/aenv
mkdir -p "$PROJECT"

$BIN --version
# → aenv 0.3.0
```

The rest of the commands use these three env vars.

---

## Step 1 — create the four namespaces

```bash
$BIN create base --adapter claude-code
$BIN create experiments --extends base --adapter claude-code
$BIN create detailed-execution --extends base --adapter claude-code
$BIN create analyst --extends base --adapter claude-code
```

```
Created namespace 'base' at /tmp/aenv-spec-walk-…/envs/base
Created namespace 'experiments' at /tmp/aenv-spec-walk-…/envs/experiments
Created namespace 'detailed-execution' at /tmp/aenv-spec-walk-…/envs/detailed-execution
Created namespace 'analyst' at /tmp/aenv-spec-walk-…/envs/analyst
```

```bash
$BIN list
```

```
NAME                   EXTENDS                        ADAPTERS
analyst                base                           claude-code
base                   -                              claude-code
detailed-execution     base                           claude-code
experiments            base                           claude-code
```

(A fresh registry also lists the built-in `cherny` and `karpathy` example namespaces; they're omitted from the `list` output blocks here for clarity.)

---

## Step 2 — base (the org-wide preamble)

`base` carries content that every harness inherits: universal build commands,
project-layout facts, and the two structural policies from §4.1
(`skill_requires_description`, `mcp_requires_command_or_url`).

`$AENV_HOME/envs/base/aenv.toml`:

```toml
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-sonnet-4.6"
instructions_budget = 5000

[policies]
skill_requires_description = true
mcp_requires_command_or_url = true
```

`$AENV_HOME/envs/base/CLAUDE.md`:

```markdown
## Project Facts

This is the aenv registry — a personal library of harness configurations
organized as namespaces. Source lives at `crates/aenv-core/` and
`crates/aenv-cli/`.

## Build & Test

- `cargo build --workspace`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`

## Conventions

- Keep CLAUDE.md short. Move procedural content into skills, dispositional
  content into subagents, or use `@`-imports to load secondary docs only when
  referenced.
- Every skill must have a `description` field in its frontmatter.
```

Note the short file — under 80 lines as §2 specifies. The procedural weight
lives in the skills below.

---

## Step 3 — experiments

`experiments` is loose and exploratory: two authored skills, no parameter
overrides beyond what `base` provides, no MCP extensions (§4.2).

### Manifest

`$AENV_HOME/envs/experiments/aenv.toml`:

```toml
name = "experiments"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "compare-approaches"
mode = "authored"
adapter = "claude-code"

[[skills]]
name = "quick-prototype"
mode = "authored"
adapter = "claude-code"
```

The absence of a `[parameters]` block is the statement: `experiments` inherits
`claude-sonnet-4.6` and the 5,000-char instructions budget from `base` without
overriding either. Compare to `detailed-execution` which overrides both.

### Disposition

`$AENV_HOME/envs/experiments/CLAUDE.md`:

```markdown
## Disposition

You are in exploration mode. Prefer breadth over depth. When implementing
something, offer 2–3 alternative approaches before settling on one. Defer
tests and polish unless explicitly asked. Quick sketches beat careful
architecture in this context.
```

### Skills (authored)

`$AENV_HOME/envs/experiments/.claude/skills/compare-approaches/SKILL.md`:

```markdown
---
name: compare-approaches
description: >
  Triggered when the user asks to implement something. Produces 2–3 brief
  alternative sketches before committing to any one approach. Each sketch
  names its tradeoff in one sentence.
---

When the user asks you to implement something, pause before writing any code.
Produce 2–3 alternative sketches:

1. Name the approach (e.g., "Option A: trait object dispatch").
2. Show the key signature or 5–10 lines of the core idea.
3. State the main tradeoff in one sentence.

Then ask the user which direction to pursue, or note which you recommend and
why. Only after a direction is chosen should you write the full implementation.
```

`$AENV_HOME/envs/experiments/.claude/skills/quick-prototype/SKILL.md`:

```markdown
---
name: quick-prototype
description: >
  Triggered for "try X" or "just make it work" requests. Biases toward the
  smallest-viable change that demonstrates the idea, explicitly marking
  production concerns as TODO.
---

When the user says "try X", "prototype", "just make it work", or similar:

- Make the smallest change that demonstrates the idea.
- Mark any shortcuts with `// TODO(proto): <concern>`.
- Do not refactor surrounding code unless the prototype requires it.
- Do not write tests unless the user asks.

State clearly at the end: "This is a prototype. TODOs: [list]."
```

---

## Step 4 — detailed-execution

`detailed-execution` is careful, spec-driven work: three skills (1 authored,
2 imported), heavier parameters, and MCP extended toward lint/test tooling.

### Skill sources

The spec's §4.3 `match-conventions` entry points at a fictional git URL:
`git+https://github.com/acme/aenv-skills.git#match-conventions`. This
walkthrough uses local path sources so it runs without network access. The
deviation is noted in "Honest gaps" at the end.

Create a sibling directory to serve as the import source:

```bash
export TEAM_SKILLS=/tmp/aenv-spec-walk-team-skills
mkdir -p $TEAM_SKILLS/match-conventions
mkdir -p $TEAM_SKILLS/check-before-submit
```

`$TEAM_SKILLS/match-conventions/SKILL.md`:

```markdown
---
name: match-conventions
description: >
  Triggered when modifying an existing file. Directs the agent to read 2–3
  sibling files first to absorb local naming, error-handling, and formatting
  conventions before writing any new code.
---

Before editing any existing file:

1. Read 2–3 sibling files in the same directory or module.
2. Note the naming convention (snake_case, CamelCase, prefix patterns).
3. Note the error-handling style (Result<T, E> shape, unwrap policy, ? usage).
4. Note the formatting preferences not captured by rustfmt (comment density,
   doc comment style, blank-line conventions).

Then write code that a reviewer cannot distinguish from the existing author's
style. Do not introduce new patterns without noting the deviation explicitly.
```

`$TEAM_SKILLS/check-before-submit/SKILL.md`:

```markdown
---
name: check-before-submit
description: >
  Triggered when the agent indicates it is done with a code change. Runs a
  final checklist: tests pass, types check, no debug prints, no dead code
  warnings.
---

Before declaring a code change complete, run through this checklist:

- [ ] `cargo test --workspace --all-targets` passes (or note which tests were
      skipped and why).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] No `println!`, `dbg!`, `eprintln!`, or `todo!()` left in non-test code.
- [ ] No `#[allow(dead_code)]` or `#[allow(unused)]` added without a comment.
- [ ] The change compiles with `--release` if it touches performance paths.

Report the result of each check. If any check fails, fix it before marking
done.
```

### Manifest

`$AENV_HOME/envs/detailed-execution/aenv.toml`:

```toml
name = "detailed-execution"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md"]

[adapters.mcp]
files = [".mcp.json"]

[parameters]
default_model = "claude-opus-4.7"
instructions_budget = 3000
auto_invoke_subagents = ["code-reviewer"]

[[skills]]
name = "write-tests"
mode = "authored"
adapter = "claude-code"

[[skills]]
name = "match-conventions"
mode = "imported"
adapter = "claude-code"
source = "/tmp/aenv-spec-walk-team-skills/match-conventions"

[[skills]]
name = "check-before-submit"
mode = "imported"
adapter = "claude-code"
source = "/tmp/aenv-spec-walk-team-skills/check-before-submit"
```

Two differences from §4.3: (1) `merge = "deep"` under `[adapters.mcp]` is
omitted — the current binary expects this as a map `{ ".mcp.json" = "deep" }`,
not a bare string; `.mcp.json` materializes as a symlink here instead of a
deep merge. (2) The `[[agents]]` table is omitted — the current manifest schema
does not support an `agents` array; see "Honest gaps."

### Disposition

`$AENV_HOME/envs/detailed-execution/CLAUDE.md`:

```markdown
## Disposition

You are executing against a spec or ticket. Match existing conventions; ask
before touching files outside the stated scope; tests are part of done. When
in doubt, do less and ask — conservative changes beat clever ones.

@./docs/conventions.md
```

### Authored skill

`$AENV_HOME/envs/detailed-execution/.claude/skills/write-tests/SKILL.md`:

```markdown
---
name: write-tests
description: >
  Triggered when a code change is proposed. Pushes for thorough test coverage
  of the diff: unit tests for each new function, integration tests for each
  new public interface, edge-case tests for error paths.
---

When you propose a code change:

1. List every public function or type added or modified.
2. For each, write at least one unit test covering the happy path.
3. For each error path (Result::Err, Option::None, panic-guard), write at
   least one test.
4. If the change crosses a module or crate boundary, write an integration test.

Mark tests with `#[cfg(test)]` and keep them in the same file unless the
project convention puts them in a `tests/` directory. Do not move to "done"
until the test list is exhausted.
```

### Agent and command

Even though `[[agents]]` isn't yet a manifest schema entry, the agent and
command files can still live in the namespace directory — they just aren't
registered in the manifest and won't be picked up by `aenv activate`
automatically. Author them now so the namespace directory looks correct; see
"Honest gaps" for the activation note.

`$AENV_HOME/envs/detailed-execution/.claude/agents/code-reviewer.md`:

```markdown
---
name: code-reviewer
description: >
  A fresh-context subagent invoked automatically before finalizing any diff.
  Reviews the proposed change for correctness, convention compliance, missing
  test coverage, and scope creep.
---

You are a code reviewer. You have been given a proposed diff. Your job is to:

1. Check correctness: does the logic do what the description claims?
2. Check conventions: does the code match the surrounding style?
3. Check coverage: are the happy path and error paths tested?
4. Check scope: does the change touch anything outside the stated goal?

Produce a numbered list of issues, ranked by severity (blocker / warning /
nit). If there are no issues, say "LGTM" and explain why briefly.

You do not write code. You only review and report.
```

`$AENV_HOME/envs/detailed-execution/.claude/commands/ship-it.md`:

```markdown
---
name: ship-it
description: Run final checks and produce a conventional commit message.
---

Run the pre-submit checklist (check-before-submit), then produce a commit
message in this format:

```
<type>(<scope>): <short summary under 72 chars>

<body: what changed and why, wrapped at 72 chars>

<footer: breaking changes, issue refs>
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`.

Do not commit. Print the message only, and state which checklist items passed
and which (if any) were skipped.
```

### MCP roster

`$AENV_HOME/envs/detailed-execution/.mcp.json`:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    },
    "search": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"]
    },
    "linter": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-linter"]
    }
  }
}
```

---

## Step 5 — analyst

`analyst` is read-oriented: three authored skills, a lighter model, and an
explicit tool deny-list that downstream tooling can respect (§4.4).

### Note on `cite-evidence`

The spec's §4.4 imports `cite-evidence` from `registry:cite-evidence` at
`v0.3.0`. The `registry:` source type is a forward-compat stub — the current
binary returns "not yet implemented" at activation. This walkthrough authors
`cite-evidence` locally instead. The deviation is noted in "Honest gaps."

### Manifest

`$AENV_HOME/envs/analyst/aenv.toml`:

```toml
name = "analyst"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md"]

[adapters.mcp]
files = [".mcp.json"]

[parameters]
default_model = "claude-haiku-4.5"
forbid_tools = ["edit", "write", "bash:rm", "bash:mv"]

[[skills]]
name = "trace-callgraph"
mode = "authored"
adapter = "claude-code"

[[skills]]
name = "summarize-module"
mode = "authored"
adapter = "claude-code"

[[skills]]
name = "cite-evidence"
mode = "authored"
adapter = "claude-code"
```

### Disposition

`$AENV_HOME/envs/analyst/CLAUDE.md`:

```markdown
## Disposition

You are investigating, not modifying. Cite specific file paths and line ranges
for every claim you make. Do not edit code unless explicitly asked. When
summarizing, prefer lists of findings over flowing prose.
```

### Skills (authored)

`$AENV_HOME/envs/analyst/.claude/skills/trace-callgraph/SKILL.md`:

```markdown
---
name: trace-callgraph
description: >
  Triggered when the user asks "where does X come from", "who calls Y", or
  "what's the call chain for Z". Traces the call graph upstream and downstream
  from the named symbol, citing file:line for each hop.
---

When the user asks about a call chain or symbol origin:

1. Identify the symbol (function, type, trait, macro).
2. Find its definition — cite `file:line`.
3. List every direct caller — cite each `file:line`.
4. For callers that are themselves interesting, recurse one level and note it.
5. List every direct callee — cite each `file:line`.

Present as a tree indented by call depth. Stop at 3 levels unless the user
asks to go deeper.
```

`$AENV_HOME/envs/analyst/.claude/skills/summarize-module/SKILL.md`:

```markdown
---
name: summarize-module
description: >
  Triggered when the user references a directory or module by name (e.g., "what
  does the resolver module do?"). Produces a concise summary: purpose, key
  types, key functions, dependencies in/out.
---

When the user names a directory or module:

1. List the files in the module.
2. State the module's purpose in one sentence.
3. List the key public types and their role (1 line each).
4. List the key public functions and their role (1 line each).
5. State what the module depends on (imports from outside itself).
6. State what depends on this module (who imports it).

Cite file paths for every item. Do not reproduce source code unless asked.
```

`$AENV_HOME/envs/analyst/.claude/skills/cite-evidence/SKILL.md`:

```markdown
---
name: cite-evidence
description: >
  Triggered when producing a written finding. Every factual claim must be
  followed by a citation in the form `(file:line)` or `(file:line–line)`.
  Unsupported claims should be flagged explicitly.
---

When writing up a finding or explanation:

- After every factual claim, add a parenthetical citation: `(path/to/file:42)`
  or `(path/to/file:42–51)` for a range.
- If you cannot find a source for a claim, write `(source needed)` — do not
  omit the marker.
- At the end of the finding, add a "Sources" list with each cited file and
  a one-line description of what it contributes.
- Do not state conclusions not supported by a cited file.
```

### Command

`$AENV_HOME/envs/analyst/.claude/commands/explain.md`:

```markdown
---
name: explain
description: Explain a file, function, or concept with file:line citations.
---

Explain the named file, function, or concept:

1. State what it is and why it exists (1–2 sentences).
2. Explain how it works, citing `file:line` for each mechanism.
3. Note any non-obvious design decisions and why they were made (cite evidence
   or note if speculative).
4. List callers or consumers if relevant.

Keep the explanation under 400 words unless the user asks for more detail.
```

### MCP roster (read-oriented)

`$AENV_HOME/envs/analyst/.mcp.json`:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    },
    "search": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"]
    }
  }
}
```

No linter or test-runner servers — read-orientation is expressed by omission.

---

## Step 6 — verify with `aenv list`, `aenv doctor`, `aenv skill list`

```bash
$BIN list
```

```
NAME                   EXTENDS                        ADAPTERS
analyst                base                           claude-code, mcp
base                   -                              claude-code
detailed-execution     base                           claude-code, mcp
experiments            base                           claude-code
```

```bash
$BIN list --json
```

```json
[
  {
    "name": "analyst",
    "extends": ["base"],
    "adapters": ["claude-code", "mcp"],
    "parameters_declared": ["default_model", "forbid_tools"],
    "policies_declared": [],
    "resolved_hash": "sha256-v1:f70c93e16edaf4f7972dc30eeb39a03c24f2ed4daba31e990744580227b0256d"
  },
  {
    "name": "base",
    "extends": [],
    "adapters": ["claude-code"],
    "parameters_declared": ["default_model", "instructions_budget"],
    "policies_declared": ["mcp_requires_command_or_url", "skill_requires_description"],
    "resolved_hash": "sha256-v1:f9e533093e78320ac2c9da1c834f99e5d580614edc1c986f4bc0ca843b0fde03"
  },
  {
    "name": "detailed-execution",
    "extends": ["base"],
    "adapters": ["claude-code", "mcp"],
    "parameters_declared": ["auto_invoke_subagents", "default_model", "instructions_budget"],
    "policies_declared": [],
    "resolved_hash": "sha256-v1:e0ed880142f4798e2bbdcc774f65ea42c6a824daac5bf5345f53de47135366da"
  },
  {
    "name": "experiments",
    "extends": ["base"],
    "adapters": ["claude-code"],
    "parameters_declared": [],
    "policies_declared": [],
    "resolved_hash": "sha256-v1:0908c9cb10468138451b5bd6633810db9e3e1e05bc21f30d265022b6ada2ea80"
  }
]
```

All four namespaces resolve with distinct hashes. `base` appears first because
the list is sorted alphabetically — `analyst` comes before `base` in the
output above, which is correct output (sorted by name).

Doctor check for each:

```bash
$BIN doctor base
```

```
Namespace 'base' (resolution: base)

Active policies (after inheritance):
  instructions_max_chars         = 5000 (from base)
  mcp_requires_command_or_url    = true (from base)
  skill_requires_description     = true (from base)

3 pass, 0 warn, 0 fail, 0 skipped.
No issues found.
```

```bash
$BIN doctor experiments
```

```
Namespace 'experiments' (resolution: base → experiments)

Active policies (after inheritance):
  instructions_max_chars         = 5000 (from experiments)
  mcp_requires_command_or_url    = true (from base, inherited)
  skill_requires_description     = true (from base, inherited)

5 pass, 0 warn, 0 fail, 0 skipped.
No issues found.
```

```bash
$BIN doctor detailed-execution
```

```
Namespace 'detailed-execution' (resolution: base → detailed-execution)

Active policies (after inheritance):
  instructions_max_chars         = 5000 (from detailed-execution)
  mcp_requires_command_or_url    = true (from base, inherited)
  skill_requires_description     = true (from base, inherited)

6 pass, 0 warn, 0 fail, 0 skipped.
No issues found.
```

```bash
$BIN doctor analyst
```

```
Namespace 'analyst' (resolution: base → analyst)

Active policies (after inheritance):
  instructions_max_chars         = 5000 (from analyst)
  mcp_requires_command_or_url    = true (from base, inherited)
  skill_requires_description     = true (from base, inherited)

6 pass, 0 warn, 0 fail, 0 skipped.
No issues found.
```

Skills across all namespaces:

```bash
$BIN skill list
```

```
NAMESPACE             SKILL                           MODE        SOURCE                                                        PIN
analyst               trace-callgraph                 authored    -                                                             -
analyst               summarize-module                authored    -                                                             -
analyst               cite-evidence                   authored    -                                                             -
detailed-execution    write-tests                     authored    -                                                             -
detailed-execution    match-conventions               imported    /tmp/aenv-spec-walk-team-skills/match-conventions             (head)
detailed-execution    check-before-submit             imported    /tmp/aenv-spec-walk-team-skills/check-before-submit           (head)
experiments           compare-approaches              authored    -                                                             -
experiments           quick-prototype                 authored    -                                                             -
```

Eight skills across three namespaces — `base` declares none, and each
harness's count matches what §2 specifies.

---

## Step 7 — activate experiments, observe what materializes

```bash
$BIN use experiments --project $PROJECT
$BIN activate --project $PROJECT
```

```
Pinned /tmp/aenv-spec-walk-proj to namespace 'experiments'
Activated 'experiments' in /tmp/aenv-spec-walk-proj
  + .claude/skills/compare-approaches/SKILL.md (Symlink)
  + .claude/skills/quick-prototype/SKILL.md (Symlink)
  + CLAUDE.md (SectionMerge)
```

Three managed files: a section-merged CLAUDE.md (base + experiments sections
combined) and two skill symlinks pointing into the namespace directory.

```bash
$BIN status --project $PROJECT
```

```
Active namespace: experiments
Resolution:       base → experiments

Managed files:
  ./.claude/skills/compare-approaches/SKILL.md
      from experiments::.claude/skills/compare-approaches/SKILL.md
  ./.claude/skills/quick-prototype/SKILL.md
      from experiments::.claude/skills/quick-prototype/SKILL.md
  ./CLAUDE.md
      merged from base + experiments

Parameters:
  default_model                  = claude-sonnet-4.6 (from base)
  instructions_budget            = 5000 (from base)

Active policies:
  mcp_requires_command_or_url    = true (from base)
  skill_requires_description     = true (from base)

Skills (2 authored, 0 imported):
  experiments::.claude/skills/compare-approaches/SKILL.md  authored  -
  experiments::.claude/skills/quick-prototype/SKILL.md  authored  -
```

No MCP extension — `experiments` inherits only what `base` declares (which is
none). No subagents. Parameters are base-defaults (`sonnet-4.6`, budget 5000).
The exploration harness is deliberately light.

Check provenance of one skill:

```bash
$BIN which .claude/skills/compare-approaches/SKILL.md --project $PROJECT
```

```
Qualified name:  experiments::.claude/skills/compare-approaches/SKILL.md
Materialized at: ./.claude/skills/compare-approaches/SKILL.md
Strategy:        symlink
Source path:     /tmp/aenv-spec-walk-…/envs/experiments/.claude/skills/compare-approaches/SKILL.md
Shadows:         (nothing — no parent namespace defines this artifact)
```

---

## Step 8 — switch to detailed-execution, observe the structural difference

```bash
$BIN deactivate --project $PROJECT
$BIN use detailed-execution --project $PROJECT
$BIN activate --project $PROJECT
```

```
Deactivated namespace in /tmp/aenv-spec-walk-proj
Pinned /tmp/aenv-spec-walk-proj to namespace 'detailed-execution'
Activated 'detailed-execution' in /tmp/aenv-spec-walk-proj
  + .claude/skills/check-before-submit/SKILL.md (Symlink)
  + .claude/skills/match-conventions/SKILL.md (Symlink)
  + .claude/skills/write-tests/SKILL.md (Symlink)
  + .mcp.json (Symlink)
  + CLAUDE.md (SectionMerge)
```

Five managed files vs. three for `experiments`. The extra two are `.mcp.json`
(the heavier MCP roster) and a third skill.

```bash
$BIN status --project $PROJECT
```

```
Active namespace: detailed-execution
Resolution:       base → detailed-execution

Managed files:
  ./.claude/skills/check-before-submit/SKILL.md
      from detailed-execution::.claude/skills/check-before-submit/SKILL.md
  ./.claude/skills/match-conventions/SKILL.md
      from detailed-execution::.claude/skills/match-conventions/SKILL.md
  ./.claude/skills/write-tests/SKILL.md
      from detailed-execution::.claude/skills/write-tests/SKILL.md
  ./.mcp.json
      from detailed-execution::.mcp.json
  ./CLAUDE.md
      merged from base + detailed-execution

Parameters:
  auto_invoke_subagents          = ["code-reviewer"] (from detailed-execution)
  default_model                  = claude-opus-4.7 (from detailed-execution)
  instructions_budget            = 3000 (from detailed-execution)

Active policies:
  mcp_requires_command_or_url    = true (from base)
  skill_requires_description     = true (from base)

Skills (1 authored, 2 imported):
  detailed-execution::.claude/skills/check-before-submit/SKILL.md  imported  /tmp/aenv-spec-walk-team-skills/check-before-submit
  detailed-execution::.claude/skills/match-conventions/SKILL.md  imported  /tmp/aenv-spec-walk-team-skills/match-conventions
  detailed-execution::.claude/skills/write-tests/SKILL.md  authored  -
```

Key differences from `experiments`: heavier model (`claude-opus-4.7`), tighter
instructions budget (3000 vs 5000), `auto_invoke_subagents` declared, 2 of 3
skills are imported (float to HEAD from local path sources), and `.mcp.json`
is present.

---

## Step 9 — switch to analyst, observe the read-orientation

```bash
$BIN deactivate --project $PROJECT
$BIN use analyst --project $PROJECT
$BIN activate --project $PROJECT
```

```
Deactivated namespace in /tmp/aenv-spec-walk-proj
Pinned /tmp/aenv-spec-walk-proj to namespace 'analyst'
Activated 'analyst' in /tmp/aenv-spec-walk-proj
  + .claude/skills/cite-evidence/SKILL.md (Symlink)
  + .claude/skills/summarize-module/SKILL.md (Symlink)
  + .claude/skills/trace-callgraph/SKILL.md (Symlink)
  + .mcp.json (Symlink)
  + CLAUDE.md (SectionMerge)
```

```bash
$BIN status --project $PROJECT
```

```
Active namespace: analyst
Resolution:       base → analyst

Managed files:
  ./.claude/skills/cite-evidence/SKILL.md
      from analyst::.claude/skills/cite-evidence/SKILL.md
  ./.claude/skills/summarize-module/SKILL.md
      from analyst::.claude/skills/summarize-module/SKILL.md
  ./.claude/skills/trace-callgraph/SKILL.md
      from analyst::.claude/skills/trace-callgraph/SKILL.md
  ./.mcp.json
      from analyst::.mcp.json
  ./CLAUDE.md
      merged from base + analyst

Parameters:
  default_model                  = claude-haiku-4.5 (from analyst)
  forbid_tools                   = ["edit", "write", "bash:rm", "bash:mv"] (from analyst)
  instructions_budget            = 5000 (from base)

Active policies:
  mcp_requires_command_or_url    = true (from base)
  skill_requires_description     = true (from base)

Skills (3 authored, 0 imported):
  analyst::.claude/skills/cite-evidence/SKILL.md  authored  -
  analyst::.claude/skills/summarize-module/SKILL.md  authored  -
  analyst::.claude/skills/trace-callgraph/SKILL.md  authored  -
```

Lightest model (`claude-haiku-4.5`), `forbid_tools` parameter declared (a
deny-list downstream tooling can read), no `auto_invoke_subagents`, all skills
authored locally, `.mcp.json` limited to filesystem + search (no linter/test
runner). The read-orientation is visible at every layer.

---

## Step 10 — `aenv diff` between pairs

`aenv diff <a> <b>` shows the structural difference — not a file diff but a
comparison of what each resolved namespace provides.

```bash
$BIN diff experiments detailed-execution
```

```
Skills:
  + detailed-execution::check-before-submit
  + detailed-execution::match-conventions
  + detailed-execution::write-tests
  - experiments::compare-approaches
  - experiments::quick-prototype

Parameters:
  default_model: "claude-sonnet-4.6" → "claude-opus-4.7"
  instructions_budget: 5000 → 3000
  +auto_invoke_subagents: ["code-reviewer"]
```

The skill roster flips entirely (different tasks, different tools), the model
steps up, the budget tightens, and a new parameter appears declaring the
code-reviewer subagent. This matches the §5.6 spec excerpt verbatim except for
the absent `Instructions (CLAUDE.md, section-merged)` block — the binary's
diff output focuses on parameters and skills; section-level instruction diffs
are not yet surfaced.

```bash
$BIN diff detailed-execution analyst
```

```
Skills:
  + analyst::cite-evidence
  + analyst::summarize-module
  + analyst::trace-callgraph
  - detailed-execution::check-before-submit
  - detailed-execution::match-conventions
  - detailed-execution::write-tests

Parameters:
  default_model: "claude-opus-4.7" → "claude-haiku-4.5"
  instructions_budget: 3000 → 5000
  +forbid_tools: ["edit","write","bash:rm","bash:mv"]
  -auto_invoke_subagents: ["code-reviewer"]
```

Moving from `detailed-execution` to `analyst`: model steps down, budget
relaxes, `auto_invoke_subagents` disappears, `forbid_tools` appears. The
parameters tell the same story as the disposition prose.

```bash
$BIN diff experiments analyst
```

```
Skills:
  + analyst::cite-evidence
  + analyst::summarize-module
  + analyst::trace-callgraph
  - experiments::compare-approaches
  - experiments::quick-prototype

Parameters:
  default_model: "claude-sonnet-4.6" → "claude-haiku-4.5"
  +forbid_tools: ["edit","write","bash:rm","bash:mv"]
```

The widest contrast: breadth-seeking `experiments` vs. read-only `analyst`.
Three skill swaps and a single model step-down, plus the tool deny-list
appearing. The budget doesn't change because `analyst` inherits `base`'s 5000
without overriding it.

---

## Step 11 — hash capture loop

```bash
for ns in base experiments detailed-execution analyst; do
  $BIN deactivate --project $PROJECT >/dev/null 2>&1 || true
  $BIN use $ns --project $PROJECT >/dev/null
  $BIN activate --project $PROJECT >/dev/null
  H=$($BIN status --project $PROJECT --json | python3 -c \
    "import sys,json; d=json.load(sys.stdin); print(d['resolved_hash'])")
  CHAIN=$($BIN status --project $PROJECT --json | python3 -c \
    "import sys,json; d=json.load(sys.stdin); print(' → '.join(d['resolution_chain']))")
  echo "$ns ($CHAIN): $H"
done
```

```
base (base):
  sha256-v1:f9e533093e78320ac2c9da1c834f99e5d580614edc1c986f4bc0ca843b0fde03
experiments (base → experiments):
  sha256-v1:0908c9cb10468138451b5bd6633810db9e3e1e05bc21f30d265022b6ada2ea80
detailed-execution (base → detailed-execution):
  sha256-v1:e0ed880142f4798e2bbdcc774f65ea42c6a824daac5bf5345f53de47135366da
analyst (base → analyst):
  sha256-v1:f70c93e16edaf4f7972dc30eeb39a03c24f2ed4daba31e990744580227b0256d
```

Four distinct hashes. Re-running the loop produces the same four values —
the hash is a function of resolved content plus the effective parameter map,
not of activation timestamps or filesystem state.

---

## Step 12 — cleanup

```bash
$BIN unpin --project $PROJECT
```

```
Deactivated namespace in /tmp/aenv-spec-walk-proj.
Unpinned /tmp/aenv-spec-walk-proj (was 'analyst').
```

```bash
rm -rf $AENV_HOME $TEAM_SKILLS $PROJECT
```

The project and the scratch registry are gone.

---

## Honest gaps surfaced

These are places where the spec describes something that doesn't quite work
today, discovered during the end-to-end run of this walkthrough.

### 1. `merge = "deep"` under `[adapters.mcp]` *(fixed in Phase 5.5 Track 1)*

The functional spec §4.3 and §4.4 show:

```toml
[adapters.mcp]
files = [".mcp.json"]
merge = "deep"
```

**This gap is resolved.** The manifest parser now accepts both the bare-string
form (`merge = "deep"`) and the per-file map form
(`merge = { ".mcp.json" = "deep" }`). The bare string is expanded at parse
time to apply the named strategy to every file in the adapter's `files` list,
so the public `AdapterEntry.merge` field stays `Option<BTreeMap<String,
String>>` and downstream resolver code is unchanged.

Prior to the fix, using `merge = "deep"` produced `exit 12`:

```
TOML parse error at line 9, column 9
  |
9 | merge = "deep"
  |         ^^^^^^
invalid type: string "deep", expected a map
```

This walkthrough was written before the fix and omits the `merge` key; an
updated walkthrough run would show `.mcp.json` materializing as a deep-merged
file rather than a symlink.

### 2. `[[agents]]` declarations not in the manifest schema

The spec §4.3 shows:

```toml
[[agents]]
name = "code-reviewer"
mode = "authored"
adapter = "claude-code"
```

The current `AenvManifest` struct has no `agents` field. Adding this table
produces a TOML parse error. The `code-reviewer.md` file is authored in the
namespace directory but is not registered in the manifest and will not be
materialized by `aenv activate`. It can be manually dropped into
`.claude/agents/` by other means, but `aenv` doesn't manage it.

The `auto_invoke_subagents = ["code-reviewer"]` parameter is parsed correctly
— it's a typed parameter the manifest schema accepts. The gap is that the
claude-code adapter doesn't yet project it into `.claude/settings.json`, so
declaring it changes the resolved hash but has no behavioral effect today.

### 3. `registry:cite-evidence` source type

The spec §4.4 imports `cite-evidence` from `registry:cite-evidence` at
`v0.3.0`. The `registry:` source type is a forward-compat stub — calling
`aenv activate` with a skill declared `source = "registry:cite-evidence"`
produces:

```
warning: skipping skill 'cite-evidence': registry source not yet implemented
```

The walkthrough authors `cite-evidence` locally as `mode = "authored"` instead.
This means the skill is present and functional but is not the pinned,
registry-sourced version the spec describes. When the Phase 4 registry source
is implemented, the manifest entry can be changed to `mode = "imported"` with
`source = "registry:cite-evidence"` and `ref = "v0.3.0"` without touching the
SKILL.md body.

### 4. `forbid_tools` is a parameter, not an enforced policy

`forbid_tools = ["edit", "write", "bash:rm", "bash:mv"]` is declared in
`analyst/aenv.toml` and round-trips correctly through `aenv status --json`.
The parameter changes the resolved hash when modified. But `aenv` does not
enforce it — the spec is explicit about this: "`aenv`'s role is to make the
parameter addressable and reproducible across namespaces; enforcement is
somebody else's problem." No gap here relative to the spec, but worth noting
for downstream consumers: reading this value from `aenv get .forbid_tools` is
correct; expecting `aenv activate` to block write tools is not.

### 5. `aenv diff` doesn't surface instruction-section differences

The spec §5.6 shows `aenv diff` output with an "Instructions (CLAUDE.md,
section-merged from base in both)" block that notes whether sections are
identical or differ. The current binary's diff output covers skills and
parameters but not CLAUDE.md section comparisons. This is a future surface.

### 6. Skill imports in this walkthrough use local path, not git

The spec §4.3 pins `match-conventions` to a git URL:

```toml
source = "git+https://github.com/acme/aenv-skills.git#match-conventions"
ref = "v1.2.0"
```

This walkthrough uses `source = "/tmp/aenv-spec-walk-team-skills/match-conventions"`
(a local path) for two reasons: (1) the URL is fictional, (2) git imports are
tested in their own suite and would add network latency to this demo. The
local-path source is a supported import mode with identical materialization
behavior; the only difference is that `PIN` shows `(head)` instead of a
resolved git ref.

---

## What you've just confirmed

- §2.1–§2.3: all three harnesses stand up with their specified skill rosters,
  subagent/command file layouts, and parameter/MCP shapes.
- §4.1–§4.4: manifests follow the spec schema (with the `merge = "deep"` and
  `[[agents]]` deviations documented above).
- §5.3: switching between harnesses on one project — `deactivate` + `use` +
  `activate` — cleanly removes one harness's files and materializes the next.
- §5.6: `aenv diff <a> <b>` surfaces the structural differences the spec calls
  out (skill roster changes, parameter overrides, parameter additions/removals).
- §7.3: four distinct `resolved_hash` values, stable across re-activation.

The whole sequence runs in under 30 seconds on a developer laptop. The setup
(Steps 2–5) is the manual step; once the registry is populated, every
activation/deactivation/diff is a single command.
