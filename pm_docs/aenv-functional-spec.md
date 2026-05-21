# Functional Spec: aenv

**Companion to:** PRD v0.1
**Status:** Draft
**Last updated:** 2026-05-19

This document describes how `aenv` behaves from a user's perspective. It walks through three concrete harness configurations — *experiments*, *detailed execution*, and *analyst* — and shows how each is composed, activated, and used. Where the PRD says "the system shall," this spec says "the user types this and sees that."

---

## 1. Mental model

A user has three things:

1. **A registry of namespaces** at `~/.aenv/envs/`, each a directory of harness config files plus a manifest declaring its adapters, parameters, policies, and inherited namespaces.
2. **A project** containing source code and a `.aenv` file naming which namespace(s) to activate.
3. **A shell hook** that auto-activates the right namespace when they `cd` into the project.

The word *namespace* matters here. Each named bundle is a namespace in two senses: it scopes a coherent set of skills, agents, rules, and parameters under a single name; and `aenv` preserves that scoping internally, so that the `write-tests` skill in `experiments` is unambiguously distinct from the `write-tests` skill in `detailed-execution`, even though both materialize to the same short name on disk. The agent itself has no namespace awareness — it sees a flat directory of skills. But `aenv` always knows which namespace each artifact came from, and surfaces that in `aenv which`, `aenv status --json`, and provenance output. This matters for debugging ("why is this skill behaving differently than I expected?") and for downstream tools that need to attribute runs to specific harness configurations.

The registry is the user's personal (or team-shared) library of harness configurations, organized as namespaces. Projects are consumers. Activation is the bridge that materializes a namespace's contents into a project at short names the target tool can read.

## 2. The three example harnesses

These are illustrative configurations a single user or team might maintain. They're chosen to show three distinct usage shapes — and to embody a design principle: **CLAUDE.md (and instructions files generally) should be short, durable, and always-on; the meaningful differentiation between harnesses lives in the skill/agent roster.**

This principle reflects how the major coding agents have converged. Skills, subagents, and slash commands load only when their trigger fires, while instructions files load on every turn. Putting situational guidance in CLAUDE.md crowds the context window with content that doesn't apply to most tasks. The harnesses below therefore keep CLAUDE.md to a paragraph or two and carry the procedural weight in skills and subagents.

A harness is also *structurally* different from another harness — not just different content under identical scaffolding. The `analyst` harness has no subagents; the `detailed-execution` harness ships a code-reviewer subagent that the others don't. This is intentional.

### 2.1 `experiments` harness

**Purpose:** Loose, exploratory, willing-to-be-wrong. The user is trying things, running variations, comparing approaches. They want the agent to be quick, suggest alternatives, and not get bogged down in ceremony.

**Disposition (CLAUDE.md, ~80 lines):** Sets one paragraph of orientation: "you're in exploration mode; prefer breadth over depth; offer 2–3 approaches when implementing; defer tests and polish unless asked." Plus build/test commands and where the code lives.

**Skills (the differentiation):**
- `.claude/skills/compare-approaches/` — triggered when the user asks to implement something; produces 2–3 alternative sketches.
- `.claude/skills/quick-prototype/` — triggered for "try X" requests; biases toward smallest-viable-change.

**Subagents:** None. Subagents add ceremony, which is the opposite of what this harness is for.

**Slash commands:** None custom.

**MCP:** Minimal — filesystem and search only. No linters, no test runners.

### 2.2 `detailed-execution` harness

**Purpose:** The user has a spec or ticket and wants it implemented carefully. Tests, types, conventions, code review readiness. The agent should be conservative, explicit, and ask before touching unrelated files.

**Disposition (CLAUDE.md, ~100 lines):** One paragraph: "you're executing against a spec; match existing conventions; ask before scope creep; tests are part of done." Plus build/test/lint commands and a `@./docs/conventions.md` import for the longer style guide.

**Skills:**
- `.claude/skills/write-tests/` — triggered when a code change is proposed; pushes for thorough coverage of the diff.
- `.claude/skills/match-conventions/` — triggered when modifying an existing file; directs the agent to read sibling code first.
- `.claude/skills/check-before-submit/` — triggered when the agent indicates it's done; runs a final checklist (tests pass, types check, no debug prints).

**Subagents:**
- `.claude/agents/code-reviewer.md` — invoked automatically before finalizing a diff. A fresh-context subagent that critiques the proposed change.

**Slash commands:**
- `.claude/commands/ship-it.md` — runs final checks and produces a commit message.

**MCP:** Heavier — adds linters, type-checkers, and test runners. The agent has tools to actually verify its work.

### 2.3 `analyst` harness

**Purpose:** The user is investigating, not modifying. Reading a codebase to understand it, tracing how something works, writing up findings. The agent should explain, summarize, and cite files — and should not edit anything.

**Disposition (CLAUDE.md, ~60 lines):** One paragraph: "you're investigating, not modifying; cite specific file paths and line ranges; do not edit code unless explicitly asked." Plus where the code lives.

**Skills:**
- `.claude/skills/trace-callgraph/` — triggered when the user asks "where does X come from" or similar.
- `.claude/skills/summarize-module/` — triggered when the user references a directory or module by name.
- `.claude/skills/cite-evidence/` — triggered when producing a written finding; produces output with inline file:line citations.

**Subagents:** None.

**Slash commands:**
- `.claude/commands/explain.md` — explain a file, function, or concept with citations.

**MCP:** Biased toward read tools — code search, docs lookup. Write-capable tools removed where the adapter allows.

### Notice what's different

All three harnesses have short CLAUDE.md files. The differentiation is entirely in the skill/subagent/command roster:

| Surface | experiments | detailed-execution | analyst |
|---|---|---|---|
| CLAUDE.md size | ~80 lines | ~100 lines | ~60 lines |
| Skills | 2 | 3 | 3 |
| Subagents | 0 | 1 (code-reviewer) | 0 |
| Slash commands | 0 | 1 (`/ship-it`) | 1 (`/explain`) |
| MCP weight | minimal | heavy (lint/test) | read-oriented |

This is the shape `aenv` is designed to make tractable: harnesses that meaningfully differ in their *capabilities*, not just in the tone of their instructions.

---

## 3. Registry layout

```
~/.aenv/
├── config.toml                  # global config (remotes, default hook shell)
├── envs/
│   ├── base/
│   │   ├── aenv.toml
│   │   └── CLAUDE.md            # org-wide preamble all harnesses inherit
│   ├── experiments/
│   │   ├── aenv.toml
│   │   ├── CLAUDE.md            # disposition paragraph
│   │   └── .claude/skills/
│   │       ├── compare-approaches/SKILL.md
│   │       └── quick-prototype/SKILL.md
│   ├── detailed-execution/
│   │   ├── aenv.toml
│   │   ├── CLAUDE.md            # disposition paragraph
│   │   ├── .claude/skills/
│   │   │   ├── write-tests/SKILL.md
│   │   │   ├── match-conventions/SKILL.md
│   │   │   └── check-before-submit/SKILL.md
│   │   ├── .claude/agents/
│   │   │   └── code-reviewer.md
│   │   ├── .claude/commands/
│   │   │   └── ship-it.md
│   │   └── .mcp.json            # heavier MCP roster
│   └── analyst/
│       ├── aenv.toml
│       ├── CLAUDE.md            # disposition paragraph
│       ├── .claude/skills/
│       │   ├── trace-callgraph/SKILL.md
│       │   ├── summarize-module/SKILL.md
│       │   └── cite-evidence/SKILL.md
│       ├── .claude/commands/
│       │   └── explain.md
│       └── .mcp.json            # read-oriented MCP
└── adapters/
    ├── claude-code.toml         # built-in
    ├── cursor.toml              # built-in
    ├── windsurf.toml            # built-in
    └── mcp.toml                 # built-in
```

The `base` namespace carries org-wide content (one paragraph of universal disposition, build/test commands that apply everywhere). The three task harnesses all `extends = ["base"]`. Because instructions files default to section-merge, editing the `## Build & Test` section in `base/CLAUDE.md` updates all three on next activation without overwriting each harness's disposition section.

Parameters and policies live in `aenv.toml`, not as separate files in the namespace tree. A namespace's manifest is the single point of truth for everything that isn't a materialized config file: which adapters it uses, what it extends, what skills and agents it provides, what typed configuration values it declares (`[parameters]`), and what validation rules it enforces (`[policies]`). The directory layout above shows only the materialized content; the manifest examples in §4 show the parameter and policy declarations.

## 4. Manifest examples

### 4.1 `base/aenv.toml`

```toml
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

# Parameters: typed configuration inherited by children.
# Adapters read these to influence what they materialize.
[parameters]
default_model = "claude-sonnet-4.6"
instructions_budget = 5000

# Policies: validation rules enforced by `aenv doctor`,
# also inherited.
[policies]
skill_requires_description = true
mcp_requires_command_or_url = true
```

The `base` namespace declares conservative defaults: a 5,000-character soft limit on instructions files, a default model selection that adapters can project into their tool-specific settings, and two structural policies that all child namespaces inherit. Children can override values, but they can't silently weaken inherited policies — see PRD R-82.

### 4.2 `experiments/aenv.toml`

```toml
name = "experiments"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md", ".claude/skills/"]

# Authored skills (live in this namespace's directory tree)
[[skills]]
name = "compare-approaches"
mode = "authored"
adapter = "claude-code"

[[skills]]
name = "quick-prototype"
mode = "authored"
adapter = "claude-code"
```

No `[parameters]` block, no `[policies]` block, no `[adapters.mcp]` block. The `experiments` namespace inherits everything from `base` — including the 5,000-character instructions budget and the default model — and adds only a pair of authored skills. The absence of a parameter override is itself a statement: this harness is for exploratory work where the user wants fast, conventional behavior and no extra ceremony. Compare to `detailed-execution` below, which overrides three parameters and adds heavier MCP.

### 4.3 `detailed-execution/aenv.toml`

```toml
name = "detailed-execution"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md", ".claude/skills/", ".claude/agents/", ".claude/commands/"]

[adapters.mcp]
files = [".mcp.json"]
merge = "deep"

# Parameters override base where the disposition calls for it
[parameters]
default_model = "claude-opus-4.7"           # stronger model for careful work
instructions_budget = 3000                  # stricter budget than base 5000
auto_invoke_subagents = ["code-reviewer"]   # consumed by the claude-code adapter

# Mix of authored and imported skills
[[skills]]
name = "write-tests"
mode = "authored"
adapter = "claude-code"

[[skills]]
name = "match-conventions"
mode = "imported"
adapter = "claude-code"
source = "git+https://github.com/acme/aenv-skills.git#match-conventions"
ref = "v1.2.0"   # pinned

[[skills]]
name = "check-before-submit"
mode = "imported"
adapter = "claude-code"
source = "~/team-skills/check-before-submit"
# no ref — resolves to current head, recorded in state at activation

[[agents]]
name = "code-reviewer"
mode = "authored"
adapter = "claude-code"
```

The parameter overrides are the main change. `detailed-execution` inherits `skill_requires_description = true` and `mcp_requires_command_or_url = true` from `base`, but overrides `default_model` (heavier model for high-stakes work) and `instructions_budget` (tighter limit, since this harness explicitly leans on skills rather than instructions). It also introduces a new parameter, `auto_invoke_subagents`, which the claude-code adapter knows to project into `.claude/settings.json` so the code-reviewer subagent fires automatically before finalizing changes.

Notice the mix: `write-tests` is authored locally (this namespace contains the SKILL.md), `match-conventions` is imported from a git repository and pinned to v1.2.0, `check-before-submit` is imported from a sibling directory on disk and floats to HEAD. The activation state records exactly which ref each imported skill resolved to.

### 4.4 `analyst/aenv.toml`

```toml
name = "analyst"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md", ".claude/skills/", ".claude/commands/"]

[adapters.mcp]
files = [".mcp.json"]
merge = "deep"

# A read-oriented harness states its disposition through parameters too:
# light model is fine, the work is comprehension not generation; deny-list
# of write-shaped tools that downstream adapters / wrappers may consume.
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
mode = "imported"
adapter = "claude-code"
source = "registry:cite-evidence"   # forward-compat hook for a future skill registry
ref = "v0.3.0"
```

The three manifests are *not* structurally identical, and the parameter blocks underscore that point. `experiments` inherits everything from `base` and adds nothing — the absence of overrides is the statement. `detailed-execution` overrides toward more care (heavier model, tighter instructions budget, auto-invoked code-reviewer subagent). `analyst` overrides toward read-orientation (lighter model, an explicit tool deny-list that downstream tooling can respect). On the adapter side, `detailed-execution` declares `agents/` and `commands/`; `analyst` declares `commands/` but not `agents/`; `experiments` declares neither. The skill roster differs in count, in authored-vs-imported mix, and in pinning policy. **The shape of the harness — files, skills, parameters, and policies together — reflects what the harness is for.**

Note that `forbid_tools` is not a built-in `aenv` policy. It's a parameter that downstream consumers (the eval project, IDE plugins, or a tool-permission gateway) can read and act on. `aenv`'s role is to make the parameter addressable and reproducible across namespaces; enforcement is somebody else's problem. This separation — `aenv` declares structure, downstream tools enforce semantics — is the boundary that keeps `aenv` from sprawling into being a security tool.

---

## 5. User journeys

### 5.1 First-time setup

```bash
# Install aenv (assumed)
$ aenv init-shell zsh >> ~/.zshrc
$ source ~/.zshrc

# Create the base env
$ aenv create base
Created namespace 'base' at ~/.aenv/envs/base/
$ aenv edit base
# (opens ~/.aenv/envs/base/ in $EDITOR)

# Create the three harnesses
$ aenv create experiments --extends base
$ aenv create detailed-execution --extends base
$ aenv create analyst --extends base

$ aenv list
NAME                EXTENDS    ADAPTERS
base                -          claude, cursor
experiments         base       claude, cursor, mcp
detailed-execution  base       claude, cursor, mcp
analyst             base       claude, cursor, mcp
```

### 5.2 Pinning a project

```bash
$ cd ~/code/payments-api
$ aenv use detailed-execution
Pinned ~/code/payments-api to namespace 'detailed-execution'
Activating...
  + CLAUDE.md                                  (from detailed-execution)
  + .claude/skills/write-tests/SKILL.md        (from detailed-execution)
  + .claude/skills/match-conventions/SKILL.md  (from detailed-execution)
  + .claude/agents/code-reviewer.md            (from detailed-execution)
  + .cursorrules                                (from detailed-execution)
  + .mcp.json                                   (merged from base + detailed-execution)
Backed up 1 file to .aenv-state/backup/2026-05-19T14-22-03/
  - CLAUDE.md (original preserved)
```

The existing `CLAUDE.md` was backed up before the symlink was created. The user can `aenv restore` to get it back.

### 5.3 Switching harnesses on the same project

This is the killer scenario — the one that's hard without `aenv`.

```bash
$ cd ~/code/payments-api
$ aenv status
Active namespace: detailed-execution
Resolution: base → detailed-execution
Managed files: 6
```

The user finishes a careful implementation. Now they want to explore a refactor — different disposition.

```bash
$ aenv use experiments
Deactivating 'detailed-execution'...
  - removed 6 symlinks
  - restored 0 backups (no underlying files were displaced)
Activating 'experiments'...
  + CLAUDE.md                                   (from experiments)
  + .claude/skills/compare-approaches/SKILL.md  (from experiments)
  + .cursorrules                                 (from experiments)
  + .mcp.json                                    (merged from base + experiments)

$ # Open Claude Code — it now reads the experiments CLAUDE.md
```

The codebase is unchanged. Only the harness flipped. The user can run the same prompt under both envs and observe how behavior differs.

### 5.4 Auto-activation across projects

```bash
$ cd ~/code/payments-api          # has .aenv: detailed-execution
[aenv] activated detailed-execution

$ cd ~/code/internal-tools-audit  # has .aenv: analyst
[aenv] deactivated detailed-execution
[aenv] activated analyst

$ cd ~                            # no .aenv anywhere up the tree
[aenv] deactivated analyst
```

The shell hook prints one line per state change. Silent mode is configurable.

### 5.5 Inspecting provenance

A user opens a project and wonders why a particular skill is there.

```bash
$ aenv which .claude/skills/match-conventions/SKILL.md
Qualified name:  detailed-execution::match-conventions
Materialized at: ./.claude/skills/match-conventions/SKILL.md
Source path:     ~/.aenv/envs/detailed-execution/.claude/skills/match-conventions/SKILL.md
Shadows:         (nothing — no parent namespace defines this skill)
```

When two namespaces in the resolution chain define a skill with the same short name, `aenv which` shows the shadow chain explicitly. Suppose `base` also defined a `write-tests` skill:

```bash
$ aenv which .claude/skills/write-tests/SKILL.md
Qualified name:  detailed-execution::write-tests
Materialized at: ./.claude/skills/write-tests/SKILL.md
Source path:     ~/.aenv/envs/detailed-execution/.claude/skills/write-tests/SKILL.md
Shadows:         base::write-tests
                 (at ~/.aenv/envs/base/.claude/skills/write-tests/SKILL.md)
```

The user can immediately see which version is active and where to look if they want to compare against the parent. Deep-merged files report all contributing namespaces:

```bash
$ aenv which .mcp.json
Qualified name:  (merged from base + detailed-execution)
Materialized at: ./.mcp.json (regular file, regenerated on activation)
Strategy:        deep-merge (json)
Contributors:    base::.mcp.json, detailed-execution::.mcp.json
```

Querying parameters works the same way:

```bash
$ aenv get .default_model
claude-opus-4.7
  source: detailed-execution (overrides base which declared claude-sonnet-4.6)

$ aenv get .instructions_budget
3000
  source: detailed-execution (overrides base which declared 5000)

$ aenv get .skill_requires_description
true
  source: base (inherited, not overridden)
```

### 5.6 Diff against the namespace, and between namespaces

The user edited `CLAUDE.md` in the project, forgetting it was symlinked. The edit went into the namespace itself. `aenv diff` clarifies the situation.

```bash
$ aenv diff
No drift detected. All managed files match their namespace source.

# (vs. a copy-managed file that was edited locally:)
$ aenv diff
.mcp.json (merged file, regenerated on activation)
  Local edits will be lost on next activation. Consider:
    aenv fork .mcp.json      # detach this file from namespace management
    aenv promote .mcp.json   # push your local changes back to the namespace
```

`aenv diff` also accepts two namespace names, in which case it reports the *structural* difference between them — not a file diff, but a comparison of what each namespace provides:

```bash
$ aenv diff experiments detailed-execution

Skills:
  + detailed-execution::write-tests
  + detailed-execution::match-conventions
  + detailed-execution::check-before-submit
  - experiments::compare-approaches
  - experiments::quick-prototype

Agents:
  + detailed-execution::code-reviewer

Parameters:
  default_model:       claude-sonnet-4.6   →  claude-opus-4.7
  instructions_budget: 5000                →  3000
  auto_invoke_subagents:  (unset)          →  ["code-reviewer"]

Instructions (CLAUDE.md, section-merged from base in both):
  ## Project Facts        (identical)
  ## Build & Test         (identical)
  ## Disposition          (differs — experiments emphasizes breadth; detailed-execution emphasizes care)
```

This is the comparison primitive that makes the experimentation use case real. Without it, comparing two namespaces means reading two CLAUDE.md files side-by-side and hoping you spotted what mattered.

### 5.7 Forking to a project-local override

The user wants to keep the `detailed-execution` harness mostly intact, but tweak `CLAUDE.md` for this specific repo.

```bash
$ aenv fork CLAUDE.md
Forked CLAUDE.md:
  - replaced symlink with a copy at ./CLAUDE.md
  - removed from namespace management for this project
  - subsequent activations will not touch this file
```

Now the project's `CLAUDE.md` is independent. Other files remain namespace-managed.

### 5.8 Sharing with a teammate

```bash
$ aenv sync
Pushed envs to git@github.com:acme/aenv-registry.git
  base                 (3 files changed)
  detailed-execution   (no changes)
  experiments          (no changes)
  analyst              (no changes)
```

A teammate, on a fresh machine:

```bash
$ aenv remote add team git@github.com:acme/aenv-registry.git
$ git clone git@github.com:acme/payments-api.git
$ cd payments-api
[aenv] namespace 'detailed-execution' not found locally
[aenv] hint: run 'aenv install' to fetch from configured remotes

$ aenv install
Fetching 'detailed-execution' from remote 'team'...
Also fetching dependency 'base'...
Resolving imported skills...
  - match-conventions @ git+https://github.com/acme/aenv-skills.git#match-conventions (v1.2.0) — cloned
  - check-before-submit @ ~/team-skills/check-before-submit — not found locally
    warning: imported skill 'check-before-submit' could not be resolved; activation will skip it
Installed: base, detailed-execution
Activating detailed-execution...
```

### 5.9 Authoring a new skill

The user wants to add a project-specific skill — say, one that knows how to handle their codebase's custom migration framework. They want it in `detailed-execution` so it applies whenever they're doing careful work.

```bash
$ aenv skill new run-migration --env detailed-execution --adapter claude-code
Created authored skill 'run-migration' in namespace 'detailed-execution':
  - ~/.aenv/envs/detailed-execution/.claude/skills/run-migration/SKILL.md
  - registered in ~/.aenv/envs/detailed-execution/aenv.toml

$ aenv edit detailed-execution
# (opens the namespace directory; user fills in the SKILL.md body)
```

The scaffold writes a SKILL.md with valid frontmatter for the Claude Code adapter (`name`, `description`, and a placeholder body) and appends a `[[skills]]` entry to the manifest with `mode = "authored"`. On next activation, the new skill materializes alongside the existing ones.

### 5.10 Importing an existing skill

The user wants `match-conventions` — a skill someone else maintains in a shared repo — without copying it into their namespace.

```bash
$ aenv skill import git+https://github.com/acme/aenv-skills.git#match-conventions \
    --env detailed-execution \
    --adapter claude-code \
    --pin v1.2.0
Resolving git+https://github.com/acme/aenv-skills.git#match-conventions @ v1.2.0...
Imported skill 'match-conventions' into namespace 'detailed-execution':
  - source: git+https://github.com/acme/aenv-skills.git#match-conventions
  - pinned ref: v1.2.0
  - registered in ~/.aenv/envs/detailed-execution/aenv.toml

# Or, from a local path, unpinned (floats to head):
$ aenv skill import ~/team-skills/check-before-submit \
    --env detailed-execution \
    --adapter claude-code
Imported skill 'check-before-submit' into namespace 'detailed-execution':
  - source: ~/team-skills/check-before-submit
  - no pin (will resolve on each activation)
```

At activation time, imported skills are fetched (cached for git sources) and materialized at the project's adapter-appropriate skill path. The activation state records exactly which ref was resolved:

```bash
$ aenv status --json | jq '.managed_files[] | select(.role=="skills")'
{
  "path": ".claude/skills/match-conventions/SKILL.md",
  "provided_by": "detailed-execution",
  "mode": "imported",
  "source": "git+https://github.com/acme/aenv-skills.git#match-conventions",
  "resolved_ref": "v1.2.0",
  "resolved_hash": "sha256:7e2a..."
}
```

### 5.11 Listing skills across namespaces

```bash
$ aenv skill list
ENV                  SKILL                  MODE       SOURCE                                              PIN
experiments          compare-approaches     authored   -                                                   -
experiments          quick-prototype        authored   -                                                   -
detailed-execution   write-tests            authored   -                                                   -
detailed-execution   match-conventions      imported   git+github.com/acme/aenv-skills#match-conventions   v1.2.0
detailed-execution   check-before-submit    imported   ~/team-skills/check-before-submit                   (head)
analyst              trace-callgraph        authored   -                                                   -
analyst              summarize-module       authored   -                                                   -
analyst              cite-evidence          imported   registry:cite-evidence                              v0.3.0
```

### 5.12 Running `aenv doctor`

The user has been iterating on their `detailed-execution` namespace. They want a sanity check.

```bash
$ aenv doctor detailed-execution
Namespace 'detailed-execution' (resolution: base → detailed-execution)

Active policies (after inheritance):
  instructions_max_chars       = 3000  (from detailed-execution; base had 5000)
  skill_requires_description   = true  (from base)
  mcp_requires_command_or_url  = true  (from base)

Instructions files:
  ./CLAUDE.md                  412 chars     ok (budget 3000)

Skills (3 authored, 2 imported):
  detailed-execution::write-tests          authored                ok
  detailed-execution::match-conventions    imported @ v1.2.0       ok (reachable)
  detailed-execution::check-before-submit  imported @ head         ok (reachable, resolves to 7e2a...)

Subagents (1):
  detailed-execution::code-reviewer        authored                ok

MCP servers: 4 declared, deep-merged from base + detailed-execution

No issues found.
```

With a bloated CLAUDE.md and a missing skill description, the same command surfaces both — the size warning *and* the structural policy violation:

```bash
$ aenv doctor experiments-overgrown
Namespace 'experiments-overgrown'

Active policies:
  instructions_max_chars       = 5000  (from base, inherited)
  skill_requires_description   = true  (from base, inherited)

Issues:
  ✗ POLICY violation: instructions_max_chars
    file:    ./CLAUDE.md  (8,247 chars, budget 5,000)
    hint:    refactor procedural content into skills, dispositional content
             into subagents, or use @-imports to load secondary docs only
             when referenced. Consider whether the budget itself needs
             relaxing — if so, declare instructions_max_chars in this
             namespace with override = true.

  ✗ POLICY violation: skill_requires_description
    skill:   experiments-overgrown::half-baked-skill
    file:    .claude/skills/half-baked-skill/SKILL.md
    hint:    add a 'description' field to the YAML frontmatter. The
             description tells the model when to invoke the skill.

2 policy violations. Activation is unaffected; doctor is advisory.
```

This is the principle from §2 enforced as a check: skill-heavy harnesses pass cleanly; CLAUDE.md-heavy harnesses get a nudge. Policy violations don't block activation — they're surfaced for the user (or CI) to decide what to do.

---

## 6. Comparative use: running the same task under different harnesses

This is the workflow `aenv` makes possible that ad-hoc config editing makes painful. Consider a user evaluating how harness disposition affects an agent's approach to a refactor.

```bash
$ cd ~/code/payments-api
$ aenv use experiments
$ # Run task: "Refactor the retry logic in payment_processor.py"
$ # Save the agent's output, the diff, the conversation, to ./runs/experiments-001/

$ aenv use detailed-execution
$ # Same prompt, same starting commit.
$ # Save to ./runs/detailed-execution-001/

$ aenv use analyst
$ # Same prompt — though analyst will likely refuse to modify code, which is itself a data point.
$ # Save to ./runs/analyst-001/

$ diff -r runs/experiments-001/ runs/detailed-execution-001/
```

`aenv` doesn't run the comparison or evaluate the outputs — that's out of scope, and is the natural domain of a separate downstream project. What `aenv` does is make the *inputs* reproducible and reversible so the comparison is meaningful. Without it, the user would be hand-editing CLAUDE.md three times and trying to remember what each version contained.

A downstream tool driving this workflow programmatically would use the scriptability affordances described in §7 — most importantly the `--json` output, the resolved-env content hash, and the `--project` flag — so that each run can be unambiguously attributed to a specific harness configuration even months later when the registry has evolved.

---

## 7. Scriptability: driving aenv from another tool

`aenv` is designed to be both a human CLI and a building block for downstream tools. This section shows the affordances that make it scriptable, with concrete examples a future harness-evaluation project (or any other consumer) might use.

### 7.1 Structured output

Every read-oriented command accepts `--json`. The output is the source of truth for programmatic consumers. All artifacts are addressed by their qualified name (`namespace::short_name`); shadowing relationships are explicit.

```bash
$ aenv status --json
{
  "project": "/home/user/code/payments-api",
  "active_namespace": "detailed-execution",
  "resolution_chain": ["base", "detailed-execution"],
  "resolved_hash": "sha256-v1:c4f3a8...",
  "parameters": {
    "default_model": {
      "value": "claude-opus-4.7",
      "source_namespace": "detailed-execution",
      "inheritance_chain": [
        {"namespace": "base", "value": "claude-sonnet-4.6"},
        {"namespace": "detailed-execution", "value": "claude-opus-4.7"}
      ]
    },
    "instructions_budget": {
      "value": 3000,
      "source_namespace": "detailed-execution",
      "inheritance_chain": [
        {"namespace": "base", "value": 5000},
        {"namespace": "detailed-execution", "value": 3000}
      ]
    }
  },
  "policies": {
    "skill_requires_description": {"value": true, "source_namespace": "base"},
    "instructions_max_chars": {"value": 3000, "source_namespace": "detailed-execution"}
  },
  "managed_files": [
    {
      "path": "CLAUDE.md",
      "qualified_name": "detailed-execution::CLAUDE.md",
      "short_name": "CLAUDE.md",
      "provided_by_namespace": "detailed-execution",
      "strategy": "section-merge",
      "contributors": ["base::CLAUDE.md", "detailed-execution::CLAUDE.md"],
      "shadows": []
    },
    {
      "path": ".claude/skills/write-tests/SKILL.md",
      "qualified_name": "detailed-execution::write-tests",
      "short_name": "write-tests",
      "provided_by_namespace": "detailed-execution",
      "strategy": "symlink",
      "source": "/home/user/.aenv/envs/detailed-execution/.claude/skills/write-tests/SKILL.md",
      "shadows": ["base::write-tests"]
    },
    {
      "path": ".mcp.json",
      "qualified_name": "(merged)",
      "short_name": ".mcp.json",
      "provided_by_namespace": null,
      "strategy": "deep-merge",
      "merge_kind": "json-deep",
      "contributors": ["base::.mcp.json", "detailed-execution::.mcp.json"]
    }
  ],
  "backed_up": [
    {
      "path": "CLAUDE.md",
      "backup": ".aenv-state/backup/2026-05-19T14-22-03/CLAUDE.md"
    }
  ]
}
```

```bash
$ aenv list --json
[
  {
    "name": "base",
    "extends": [],
    "adapters": ["claude-code", "cursor"],
    "parameters_declared": ["default_model", "instructions_budget"],
    "policies_declared": ["skill_requires_description", "mcp_requires_command_or_url"],
    "resolved_hash": "sha256-v1:9b2e1c..."
  },
  {
    "name": "detailed-execution",
    "extends": ["base"],
    "adapters": ["claude-code", "cursor", "mcp"],
    "parameters_declared": ["default_model", "instructions_budget", "auto_invoke_subagents"],
    "policies_declared": [],
    "resolved_hash": "sha256-v1:c4f3a8..."
  }
]
```

```bash
$ aenv get .default_model --json
{
  "parameter": "default_model",
  "value": "claude-opus-4.7",
  "source_namespace": "detailed-execution",
  "inheritance_chain": [
    {"namespace": "base", "value": "claude-sonnet-4.6"},
    {"namespace": "detailed-execution", "value": "claude-opus-4.7"}
  ]
}
```

### 7.2 Project-scoped activation without `cd`

A script driving multiple projects, or evaluating multiple harnesses in parallel, cannot rely on the shell hook. Every command accepts `--project <path>`.

```bash
$ aenv activate experiments --project /home/user/code/payments-api
$ aenv status --project /home/user/code/payments-api --json
{ "active_namespace": "experiments", ... }
$ aenv deactivate --project /home/user/code/payments-api
```

This works identically whether or not the shell hook is installed, and whether or not the project has a `.aenv` file. A `.aenv` file is the *human* convention; `--project` is the *machine* interface.

### 7.3 Resolved-namespace content hash

The `resolved_hash` is a stable identifier for the fully-resolved namespace — the union of all files after `extends` resolution, canonicalized and hashed. The full algorithm is specified in PRD §5.15; the user-facing properties are:

- Two users on different machines computing the hash from the same registry commit see the same value, byte for byte.
- The hash changes if and only if the resolved content changes. Reordering manifest entries, reformatting `aenv.toml`, or renaming sibling namespaces has no effect.
- For deep-merged files, the hash incorporates the merged contents (canonical JSON), not the source fragments. Adding a `base` and an overlay that together produce the same merged result as a single namespace produces the same hash.
- The hash is namespaced by algorithm version. Today it's emitted as `sha256-v1:<hex>`. If the canonicalization ever changes, both versions will be emitted during a deprecation window so consumers can migrate.
- The hash covers materialized content only. Parameters, policies, and shadow chains are *not* part of the hash — they're metadata about the resolution, not the resolved output. If two namespaces with different parameters produce identical materialized files, their hashes will be equal. Downstream tools that care about parameter values should record them separately (see §7.1 JSON output).

This is the affordance that lets a downstream evaluation tool record "this run used harness `detailed-execution` at hash `sha256-v1:c4f3a8...`" and later verify reproducibility — or detect that the harness has since changed.

```bash
# A downstream eval tool might do:
$ HASH=$(aenv status --project ./repo --json | jq -r .resolved_hash)
$ echo "$HASH" > runs/2026-05-19/harness.hash
```

### 7.4 Exit codes

Documented and stable across minor versions. A consumer can branch on them without parsing text.

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Generic error |
| 10 | Namespace not found |
| 11 | Adapter missing |
| 12 | Manifest invalid |
| 13 | Activation conflict (e.g. file exists, backup failed) |
| 14 | Remote unreachable |
| 15 | Cycle in `extends` chain |
| 16 | Parameter undefined (`aenv get` on a parameter not declared in the resolution chain) |
| 17 | Policy violation (e.g. child silently weakens parent policy without `override = true`) |
| 20 | Project not pinned (no `.aenv` and no `--project` resolution) |

`aenv --help` and `aenv <command> --help` list these. They are part of the public contract.

### 7.5 A scripted comparison

Bringing the pieces together — what a downstream tool's inner loop might look like, expressed as shell for clarity:

```bash
#!/usr/bin/env bash
set -euo pipefail

PROJECT=~/code/payments-api
TASK="Refactor the retry logic in payment_processor.py"

for ns in experiments detailed-execution analyst; do
  aenv activate "$ns" --project "$PROJECT"

  status=$(aenv status --project "$PROJECT" --json)
  hash=$(echo "$status" | jq -r .resolved_hash)
  outdir="runs/${ns}-${hash:0:12}"
  mkdir -p "$outdir"
  echo "$ns" > "$outdir/namespace.name"
  echo "$hash" > "$outdir/namespace.hash"

  # Capture parameter values for run attribution — the eval project
  # can later attribute behavior differences to specific parameter changes.
  echo "$status" | jq .parameters > "$outdir/namespace.parameters.json"

  # ... run the agent against $PROJECT with $TASK,
  # capture outputs into $outdir ...

  aenv deactivate --project "$PROJECT"
done
```

Note what's *not* there: no parsing of human output, no assumptions about the shell hook, no `cd` shenanigans. Every interaction with `aenv` is either a command with a known exit code or a `--json` query.

---

## 8. Commands reference

| Command | Purpose |
|---|---|
| `aenv init-shell <bash\|zsh\|fish>` | Print shell hook for sourcing in rc file |
| `aenv create <name> [--extends <name>...]` | Create a new namespace |
| `aenv delete <name>` | Delete a namespace from the registry |
| `aenv list [--json]` | List all namespaces |
| `aenv edit <name>` | Open namespace directory in `$EDITOR` |
| `aenv use <name>` | Set the current project's `.aenv` pin |
| `aenv activate [<name>] [--project <path>]` | Activate the pinned namespace, or a named namespace against a specific project |
| `aenv deactivate [--project <path>]` | Remove all materialized files, restore backups |
| `aenv status [--project <path>] [--json]` | Show active namespace, resolution chain, managed files, parameters, policies, resolved hash |
| `aenv diff [--json]` | Show drift between project files and namespace source |
| `aenv diff <namespace-a> <namespace-b> [--json]` | Structural diff between two namespaces |
| `aenv which <path> [--json]` | Show qualified name of the namespace that provided a given file, plus any shadowed predecessors |
| `aenv get <namespace>.<param> [--json]` | Print a parameter's resolved value and inheritance chain. `.` for active namespace |
| `aenv doctor [<namespace>] [--json]` | Validate a namespace (or all namespaces) against its policies |
| `aenv fork [<path>]` | Detach the project (or a single file) from namespace management |
| `aenv promote <path>` | Push project-local edits back into the namespace |
| `aenv restore` | Restore most recent backup set |
| `aenv install` | Fetch namespaces named in `.aenv` from configured remotes |
| `aenv sync` | Push/pull registry against configured remotes |
| `aenv remote add <name> <url>` | Configure a remote |
| `aenv adapter add <path>` | Install a new adapter plugin |
| `aenv adapter list [--json]` | List installed adapters |
| `aenv skill new <name> --namespace <ns>` | Scaffold a new authored skill in a namespace |
| `aenv skill import <source> --namespace <ns> [--pin]` | Add an imported skill entry to a namespace's manifest |
| `aenv skill list [--namespace <ns>] [--json]` | List skills in a namespace (or all) with authored/imported mode | List installed adapters |

| `aenv skill import <source> --env <env> --adapter <a> [--pin <ref>]` | Add an imported skill (git, path, or registry source) |
| `aenv skill list [--env <name>] [--json]` | List skills across envs with mode, source, and pin |
| `aenv doctor [--env <name>] [--json]` | Validate envs: size limits, skill reachability, manifest sanity |

The `--project <path>` flag is accepted by every command that operates on a project, not only those shown above. Read-oriented commands accept `--json` per §7.1, and use qualified names (`namespace::short_name`) for all artifacts in machine output.

---

## 9. Behavioral details worth pinning down

A few situations that don't fit cleanly into the EARS requirements but matter in practice.

**Editor reload.** When the user switches envs while Claude Code is already running, Claude won't see the new `CLAUDE.md` until it's restarted. The activator should print a hint: `Note: restart your agent to pick up the new harness.`

**`.gitignore` for `.aenv-state/`.** Activation creates `.aenv-state/state.json` and `.aenv-state/backup/`. These are project-local state and should never be committed. The `aenv use` command should add `.aenv-state/` to `.gitignore` automatically. The `.aenv` pin file itself *is* committed. The state directory is named `.aenv-state/` (rather than living under `.aenv/`) because `.aenv` is a regular file at the same path, and a file and a directory cannot share a name.

**Nested projects.** If `~/code/monorepo/.aenv` says `detailed-execution` and `~/code/monorepo/experiments/.aenv` says `experiments`, then `cd`-ing into `experiments/` activates `experiments`, not `detailed-execution`. The nearest-ancestor `.aenv` wins.

**Untracked files in namespace directories.** If the user drops a file into `~/.aenv/envs/experiments/` that isn't declared by any adapter's `files` list, `aenv` ignores it. The manifest is authoritative; the directory may contain extra files (drafts, notes) without being materialized.

**Hidden activation drift.** If a user edits a symlinked file, they're editing the namespace. This is by design (it's how `aenv promote`-style workflows feel natural), but it's a footgun. The first activation in a new shell prints a one-line reminder: `Managed files are symlinks to the namespace. Edits will affect all projects using this namespace.`

**Hash stability.** The resolved hash is computed over file contents and relative paths after `extends` resolution, in lexicographic order. It explicitly does not include the manifest itself (so reformatting `aenv.toml` doesn't perturb it) and does not include timestamps or filesystem metadata. Adapter-driven merging is performed before hashing, so a deep-merged `.mcp.json` contributes its merged contents, not its source fragments.

**Section-merge for instructions files.** By default, instructions files like `CLAUDE.md` are merged across the `extends` chain by Markdown section. A `base/CLAUDE.md` providing a `## Build & Test` section and a `detailed-execution/CLAUDE.md` providing a `## Disposition` section produce a combined file with both. If two namespaces in the chain provide the same section, content is appended in chain order. To force replacement rather than append, the later namespace marks the section with `<!-- aenv:replace -->` immediately under the heading. This is the right default because dispositional content lives best at the leaf namespace while structural content (build commands, conventions imports) lives best at `base`.

**Imported skill caching.** Git-sourced imported skills are cloned into a content-addressed cache at `~/.aenv/cache/skills/<source-hash>/<ref>/`. Subsequent activations reuse the cache. `aenv skill refresh` re-fetches unpinned imports; pinned imports are immutable once resolved.

**Imported skill failure modes.** If an imported skill's source is unreachable at activation time, the default is to skip the skill, warn, and continue. The manifest can mark a skill `required = true` to make activation fail instead. This is per-skill, not env-wide: a non-critical skill from a flaky source shouldn't break the harness, but a load-bearing one should.

---

## 10. What this spec deliberately omits

- **How adapters are implemented** — that's a design doc concern.
- **How merging is implemented for each format** — that's per-adapter and lives in adapter docs.
- **Performance characteristics of the `cd` hook** — should be sub-10ms, but specifying that is a non-functional requirement.
- **GUI, dashboards, or web UI** — out of scope for v1.
- **Evaluation tooling for comparing harnesses** — separate product, separate spec. `aenv` provides the scriptability surface (§7) that such a project would consume.
