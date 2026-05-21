# PRD: aenv — Virtual Environments for AI Coding Harness Configs

**Status:** Draft v0.3
**Author:** [you]
**Last updated:** 2026-05-19

---

## 1. Background

AI coding agents (Claude Code, Cursor, Aider, Cline, Continue, Windsurf, etc.) are configured through a sprawl of per-project files: `CLAUDE.md`, `.cursor/rules/`, `.aider.conf.yml`, `.mcp.json`, skill folders, agent definitions, prompt libraries. Today these files are managed ad-hoc — hand-edited per repo, copy-pasted across repos, and versioned inline with code. This conflates two distinct concerns: *what the code does* and *how the agent reasons about the code*.

`aenv` ("agent-env") is a tool-agnostic virtual environment system for these configs, modeled on Python's `venv` and direnv. It lets users define named, composable, version-controlled **namespaces** of harness configuration and activate them per directory.

A namespace is the unit of harness identity in aenv. Each namespace owns its skills, agents, slash commands, MCP server entries, instruction files, and parameters; composes with other namespaces via inheritance; and materializes one-at-a-time into a project. Namespaces are a build-time organizational concept — only one namespace's content is materialized into a project at any time, and the agent itself sees a flat set of files with no namespace awareness — but every artifact aenv emits, queries, and hashes carries a namespace-qualified identity so that provenance is preserved across composition and shadowing.

## 2. Goals

- Enable reproducible, swappable harness configurations across projects and machines.
- Support composition of configuration layers (base + overlay) with namespace-respecting identity for every artifact.
- Make harness provenance explicit, auditable, and unambiguously addressable.
- Remain tool-agnostic via a plugin (adapter) model.
- Encode best-practice hygiene for always-on instruction files (CLAUDE.md and equivalents) so that skill/agent/command artifacts carry the bulk of harness differentiation rather than bloating always-on context.

## 3. Non-goals

- Modifying agent runtime behavior beyond what their config files already permit.
- Providing evaluation, telemetry, or behavioral comparison of agents.
- Replacing per-tool config formats with a unified DSL.
- Managing secrets or credentials.
- Runtime namespace isolation. Namespaces resolve to a flat materialization that the agent reads without namespace awareness; aenv does not attempt to make multiple namespaces simultaneously active in the same project, and does not attempt to expose namespace prefixes to the agent at runtime.

## 4. Glossary

- **Namespace**: a named, directory-backed bundle of harness configuration — skills, agents, commands, instruction files, MCP entries, parameters, and policies. The primary user-facing concept. (Internally and in legacy contexts, also called an "env".)
- **Registry**: the local store of namespaces (default `~/.aenv/envs/`).
- **Adapter**: a plugin describing one tool's config file locations, role classifications, and merge semantics.
- **Manifest**: per-namespace metadata file (`aenv.toml`) declaring adapters, extended namespaces, merge rules, parameters, policies, and skill content declarations.
- **Project pin**: a `.aenv` file at a project root naming the namespace(s) to activate.
- **Materialization**: the act of placing namespace files into a project (via symlink or merged copy).
- **Qualified name**: a namespace-prefixed artifact identifier of the form `<namespace>::<short_name>` (e.g., `detailed-execution::write-tests`). Used in aenv's internal model, machine output, and provenance records. The agent never sees qualified names; it sees only the short name as materialized into the project.
- **Shadowing**: in a resolution chain, when an overlay namespace defines an artifact with the same short name as a parent namespace. The overlay's artifact is materialized; the parent's is recorded as shadowed but not erased from provenance.
- **Authored skill**: a skill whose content lives in the namespace's own directory tree.
- **Imported skill**: a skill whose content is materialized by reference from a local path, git source, or skill registry.
- **Instructions file**: a file classified by its adapter with role `instructions` — always-on context loaded by the agent on every turn (e.g., `CLAUDE.md`, `.cursor/rules/*.mdc`, `.windsurf/rules/*.md`).

---

## 5. Requirements (EARS)

### 5.1 Namespace lifecycle

- **R-1 (Ubiquitous)** The system shall store namespaces in a registry directory whose location defaults to `~/.aenv/envs/` and is overridable via the `AENV_HOME` environment variable.
- **R-2 (Event)** When the user runs `aenv create <name>`, the system shall create a new namespace directory containing a default `aenv.toml` manifest.
- **R-3 (Event)** When the user runs `aenv list`, the system shall print all namespaces in the registry with their names and any `extends` chain.
- **R-4 (Event)** When the user runs `aenv delete <name>`, the system shall remove the namespace from the registry only after confirming it is not currently active in any tracked project.
- **R-5 (Unwanted behavior)** If the user attempts to create a namespace with a name that already exists, the system shall reject the command and exit non-zero without modifying the registry.

### 5.2 Manifest, composition, and namespace identity

- **R-6 (Ubiquitous)** Each namespace shall contain a manifest file (`aenv.toml`) declaring its name, adapters, optional `extends` list, optional per-file merge strategies, optional skill content declarations (§5.3), optional parameters (§5.14), and optional policies (§5.15).
- **R-7 (Event)** When resolving a namespace that declares `extends`, the system shall recursively resolve parent namespaces and apply files in order according to the following defaults, which may be overridden per-file in the manifest:
  - For files marked `role = "instructions"` by their adapter, the default merge strategy shall be `section-merge`: contents are combined under matching top-level Markdown headings (`#` and `##`), with later namespaces appending to or replacing sections from earlier namespaces.
  - For files declared with `merge = "deep"` in a recognized structured format (JSON, YAML, TOML), the system shall deep-merge contents across the resolution chain.
  - For all other files, the default shall be `last-wins`: later namespaces in the chain replace earlier versions entirely.
- **R-8 (Event)** For section-merged instructions files, a section in a later namespace may be marked with the HTML comment `<!-- aenv:replace -->` immediately under its heading to force that section to replace rather than append to the same section from an earlier namespace.
- **R-9 (Ubiquitous)** Every artifact emitted by namespace resolution — skills, agents, commands, hooks, MCP server entries, instruction files, and parameters — shall carry a namespace-qualified identity of the form `<owning_namespace>::<short_name>`. The qualified identity shall be preserved through composition, materialization, hashing inputs, and all queries.
- **R-10 (Ubiquitous)** When an overlay namespace defines an artifact with the same short name as a parent namespace in the resolution chain, the system shall record both qualified identities: the overlay's identity as `provided` and the parent's identity as `shadowed`. The shadowed artifact shall not be materialized but shall remain queryable.
- **R-11 (Ubiquitous)** Short names shall be what the agent sees on disk after materialization. Qualified names shall be used internally and in machine output (`--json`). The namespace separator shall be `::`.
- **R-12 (Unwanted behavior)** If the `extends` chain contains a cycle, the system shall detect the cycle, refuse to resolve, and report the offending chain.
- **R-13 (Unwanted behavior)** If a manifest references an adapter that is not installed, the system shall report the missing adapter and refuse to activate the namespace.

### 5.3 Skill content model

Skills are first-class because adapter research established that the meaningful differentiation between harnesses lives in the skill/agent roster, not in instructions-file content.

- **R-14 (Ubiquitous)** A namespace may declare each skill it provides as either `authored` (content lives in the namespace's own directory tree) or `imported` (content is materialized by reference from an external source).
- **R-15 (Ubiquitous)** Authored skills shall be stored under the namespace directory at adapter-determined paths (for example, `~/.aenv/envs/<name>/.claude/skills/<skill>/SKILL.md`) and activated identically to any other namespace-provided file.
- **R-16 (Ubiquitous)** Imported skills shall be declared in the manifest with a `source` field naming one of: a local filesystem path, a git repository URL with optional ref, or an entry in a configured skill registry.
- **R-17 (Event)** When activating a namespace containing imported skills, the system shall resolve each `source` to its current content, copy or symlink the resolved files into the project at the adapter-determined skill path, and record provenance (source, resolved ref or content hash, qualified identity) in the activation state file.
- **R-18 (Ubiquitous)** Imported skills shall support optional pinning via a `ref` field (commit SHA, tag, or content hash). When `ref` is omitted, the system shall resolve to the source's current head and record the resolved ref in state.
- **R-19 (Event)** When the user runs `aenv skill new <name> --ns <namespace>`, the system shall scaffold a new authored skill in the named namespace: a skill directory containing a `SKILL.md` with valid adapter-appropriate frontmatter, a placeholder body, and registration in the namespace's manifest.
- **R-20 (Event)** When the user runs `aenv skill import <source> --ns <namespace>`, the system shall add an imported-skill entry to the named namespace's manifest, resolve the source, and write its current ref to the manifest if `--pin` is specified.
- **R-21 (Event)** When the user runs `aenv skill list [--ns <namespace>]`, the system shall print each skill in the named namespace (or all namespaces) with its qualified identity, mode (`authored` or `imported`), source if imported, and pinned ref if any.
- **R-22 (Unwanted behavior)** If an imported skill's source becomes unreachable at activation time, the system shall report the failure, omit that skill from activation, and continue activating the remaining files unless the manifest declares the skill as `required = true`, in which case activation fails.
- **R-23 (Ubiquitous)** Authored and imported skills shall coexist within a single namespace without restriction; a namespace may provide some skills inline and import others.

### 5.4 Instructions-file size limits

- **R-24 (State-driven)** While a namespace contains instructions files, the system shall track each file's size in characters and shall warn the user when activation would materialize an instructions file exceeding the adapter's documented soft limit.
- **R-25 (Ubiquitous)** Per-adapter soft limits shall default to 5,000 characters for general-purpose instructions files (Claude Code, Cursor, Cline, Continue, Aider) and 6,000 characters for Windsurf's `global_rules.md` and `.windsurfrules` (the documented hard limit at which the tool silently truncates).
- **R-26 (Ubiquitous)** A namespace may override the soft limit for its instructions files via the `instructions_budget` parameter (§5.14). The effective soft limit shall be the lower of the adapter's documented limit and the namespace's declared budget.
- **R-27 (Event)** When the user runs `aenv doctor [--ns <namespace>]`, the system shall report all instructions files exceeding their effective soft limit, along with hints recommending refactoring content into skills, subagents, or @-imports.
- **R-28 (Unwanted behavior)** Warnings shall not block activation. The system shall not silently truncate or modify instructions file content under any circumstance.

### 5.5 Adapters

- **R-29 (Ubiquitous)** The system shall support a plugin model in which each adapter is a single declarative file naming the tool, the project-relative paths it manages, optional merge strategies per path, role classifications (`instructions`, `settings`, `mcp`, `skills`, `agents`, `commands`, `hooks`), reload strategy, and soft size limits.
- **R-30 (Ubiquitous)** The system shall ship with built-in adapters for at minimum: Claude Code, Cursor, Aider, Cline, Continue, Windsurf, and a generic MCP config adapter.
- **R-31 (Event)** When the user runs `aenv adapter add <path>`, the system shall validate the adapter file and install it into the adapter registry.
- **R-32 (Event)** When the user runs `aenv adapter list`, the system shall print all installed adapters and the paths each manages.

### 5.6 Project pinning

- **R-33 (Ubiquitous)** A project shall declare its namespace(s) via a `.aenv` file at the project root containing one namespace name per line.
- **R-34 (Event)** When the user runs `aenv use <name>` in a project directory, the system shall write or update the `.aenv` file to name that namespace.
- **R-35 (Event)** When the user runs `aenv install` in a project containing a `.aenv` file, the system shall fetch any named namespaces not present in the local registry from configured remotes.
- **R-36 (Unwanted behavior)** If `.aenv` names a namespace that is not in the registry and cannot be fetched, the system shall warn the user, decline to activate, and continue without materializing files.

### 5.7 Activation and auto-activation

- **R-37 (Event)** When the user runs `aenv activate` in a project containing a `.aenv` file, the system shall resolve the named namespace(s) and materialize their files into the project.
- **R-38 (Event)** When the user runs `aenv deactivate` in a project with an active namespace, the system shall remove all materialized files and restore any backed-up originals.
- **R-39 (Ubiquitous)** The system shall provide shell hook scripts for bash, zsh, and fish that auto-activate on directory change.
- **R-40 (State-driven)** While the shell hook is installed and the user changes directory into a project containing a `.aenv` file, the system shall activate the named namespace if it is not already active.
- **R-41 (State-driven)** While the shell hook is installed and the user changes directory out of a project with an active namespace into a directory not covered by any `.aenv`, the system shall deactivate the namespace.
- **R-42 (State-driven)** While the shell hook is installed and the user changes directory between two projects with different `.aenv` files, the system shall deactivate the first namespace and activate the second atomically.
- **R-43 (Event)** When activation begins, the system shall write a state file (`.aenv-state/state.json`) recording the active namespace, the resolved file list with qualified identities, the shadow set, parameter values in effect, and the backup manifest. The state directory is `.aenv-state/` (not `.aenv/`) because `.aenv` is the pin file declared in R-33 and a regular file cannot coexist with a directory of the same name on a real filesystem.

### 5.8 File materialization and conflicts

- **R-44 (Event)** When materializing a file that does not exist in the project, the system shall create a symlink from the project path to the namespace file.
- **R-45 (Event)** When materializing a file whose project version differs from the namespace version and is not declared as merged, the system shall move the project file to `.aenv-state/backup/<timestamp>/` before symlinking.
- **R-46 (Event)** When materializing a file whose project version is byte-identical to the namespace version, the system shall leave the project file in place and record it as managed.
- **R-47 (Event)** When materializing a file declared with a deep-merge or section-merge strategy, the system shall compute the merged contents and write a regular (non-symlink) file, recording the source namespaces in state.
- **R-48 (Event)** When deactivating, the system shall remove only files it materialized, restore backups, and leave any files created by the user during activation untouched.
- **R-49 (Unwanted behavior)** If the system cannot write a symlink (e.g. on Windows without privilege), the adapter shall fall back to copy-mode and record the file as copy-managed in state.

### 5.9 Status and introspection

- **R-50 (Event)** When the user runs `aenv status`, the system shall print: the currently active namespace (if any), the full resolution chain, every managed file with its qualified provenance, the shadow set, the effective parameter values, and any backed-up originals.
- **R-51 (Event)** When the user runs `aenv diff`, the system shall print a unified diff between each materialized file and what the resolved namespace would produce, highlighting drift.
- **R-52 (Event)** When the user runs `aenv which <path>`, the system shall report the qualified identity of the artifact that provided the file, plus any shadowed identities from earlier namespaces in the chain.

### 5.10 Forking and divergence

- **R-53 (Event)** When the user runs `aenv fork`, the system shall replace symlinks in the project with copies, mark the project as detached from the namespace, and stop auto-activation for that project until re-pinned.
- **R-54 (Event)** When the user runs `aenv fork <name>`, the system shall create a new namespace in the registry populated with the current project's harness files.

### 5.11 Sync and sharing

- **R-55 (Ubiquitous)** The system shall support configuring one or more git remotes from which namespaces can be fetched.
- **R-56 (Event)** When the user runs `aenv sync`, the system shall pull and push the registry (or named namespaces within it) to the configured remote.
- **R-57 (Event)** When the user runs `aenv install` and the named namespace is found at multiple remotes, the system shall prefer the first configured remote and report the source.
- **R-58 (Event)** When the user runs `aenv remote add <name> <url>`, the system shall record the remote in the global configuration and make it available to subsequent `install` and `sync` operations.
- **R-59 (Event)** When the user runs `aenv promote <path>`, the system shall copy the project-local contents of that file back into the namespace that originally provided it (per its qualified identity) and re-establish the symlink.

### 5.12 Safety and reversibility

- **R-60 (Ubiquitous)** The system shall never modify a project file outside the paths declared by active adapters.
- **R-61 (Ubiquitous)** The system shall never delete a backed-up original until the user explicitly runs a cleanup command.
- **R-62 (Event)** When the user runs `aenv restore`, the system shall restore the most recent backup set for the current project, even if no namespace is currently active.
- **R-63 (Unwanted behavior)** If activation fails partway through, the system shall roll back any materialization performed in that activation attempt and leave the project in its pre-activation state.

### 5.13 Shell integration

- **R-64 (Event)** When the user runs `aenv init-shell <bash|zsh|fish>`, the system shall print to stdout a shell hook script suitable for sourcing in the corresponding rc file.
- **R-65 (Ubiquitous)** The shell hook shall be the only component that triggers auto-activation on directory change; all other commands shall function independently of whether the hook is installed.

### 5.14 Namespace parameters

Parameters are typed, namespace-scoped configuration values that adapters and validation consume during resolution and materialization. They are distinct from settings files because they are aenv-level concepts that may be projected into multiple tool-specific surfaces.

- **R-66 (Ubiquitous)** A namespace manifest may declare a `[parameters]` table containing typed key-value pairs. Supported value types shall be: string, integer, boolean, and list of strings.
- **R-67 (Event)** When resolving a namespace's `extends` chain, the system shall merge parameter tables from parent to child with last-wins semantics on a per-key basis. The effective parameter set shall be recorded in activation state (R-43).
- **R-68 (Ubiquitous)** Adapters may declare which parameters they consume and how those parameters project into tool-specific config files. The projection mapping shall be part of the adapter's declarative file.
- **R-69 (Event)** When the user runs `aenv get <namespace>.<parameter>`, the system shall print the effective value of the parameter after `extends` resolution, including the qualified identity of the namespace in the chain that provided it.
- **R-70 (Event)** When the user runs `aenv set <namespace>.<parameter> <value>`, the system shall update the named namespace's manifest to set or override the parameter.
- **R-71 (Unwanted behavior)** If a parameter declared by an adapter is referenced by a manifest with a type-incompatible value, the system shall report a manifest-invalid error and refuse to activate.

### 5.15 Namespace-scoped policies

Policies are validation rules that govern a namespace's own artifacts. They are inherited along the `extends` chain (children may add policies, may not silently disable parent `enforce` policies) and enforced by `aenv doctor`.

- **R-72 (Ubiquitous)** A namespace manifest may declare a `[policies]` table containing rule directives such as `instructions_max_chars`, `require_skill_descriptions`, `forbid_paths`, and adapter-specific policy keys.
- **R-73 (Event)** When the user runs `aenv doctor [--ns <namespace>]`, the system shall evaluate the namespace's own artifacts against the union of its own policies and all inherited policies from its `extends` chain, and shall print a report listing each policy outcome with qualified identities.
- **R-74 (Ubiquitous)** Policies are advisory by default. A namespace may mark a policy as `enforce = true`, in which case `aenv activate` of that namespace shall fail if the policy is violated.
- **R-75 (Ubiquitous)** A child namespace shall not be permitted to remove or weaken a parent's `enforce = true` policy. Child policies may only add restrictions or override advisory policies of the same key.

### 5.16 Scriptability and machine interfaces

These requirements exist to keep `aenv` usable as a building block for downstream tools (most immediately, a future harness evaluation project) without coupling it to any specific consumer.

- **R-76 (Ubiquitous)** Every read-oriented command (`status`, `list`, `which`, `diff`, `adapter list`, `skill list`, `get`, `doctor`) shall accept a `--json` flag that emits a structured representation of the same information.
- **R-77 (Ubiquitous)** All `--json` output shall use namespace-qualified identities (`<namespace>::<short_name>`) for every artifact reference. Short names shall additionally be included as a separate field where relevant for adapter consumption.
- **R-78 (Ubiquitous)** Every command that operates on the current project shall accept a `--project <path>` flag that overrides directory inference and operates on the specified path instead.
- **R-79 (Event)** When the user runs `aenv activate <name> --project <path>`, the system shall activate the named namespace against the specified project without requiring a `.aenv` file and without depending on the shell hook.
- **R-80 (Ubiquitous)** The system shall compute a stable content hash over the fully-resolved namespace (the union of all files after `extends` resolution, in a canonical order) and expose this hash via `aenv status --json` and `aenv list --json`.
- **R-81 (Ubiquitous)** The hash shall change if and only if the resolved content changes; reordering of files in the manifest, whitespace-only changes to the manifest, or unrelated registry edits shall not affect it.
- **R-82 (Ubiquitous)** The system shall use distinct non-zero exit codes for documented failure classes including at minimum: namespace not found, adapter missing, activation conflict, manifest invalid, policy-enforce failure, and remote unreachable.
- **R-83 (Ubiquitous)** Exit codes and their meanings shall be documented in the CLI help output and shall remain stable across minor versions.

### 5.17 Resolved-namespace hash specification

This is the canonical specification of the content hash referenced by R-80 and R-81. It is a public contract that downstream tools depend on; changes constitute breaking changes.

- **R-84 (Ubiquitous)** The resolved-namespace hash shall be computed as SHA-256 over a canonical byte serialization of the fully-resolved namespace, defined as follows:
  1. Resolve the namespace by applying its `extends` chain, producing a final ordered set of `(relative_path, content_bytes)` pairs in which deep-merged and section-merged files appear with their merged contents.
  2. Canonicalize the contents of structured files declared with a `merge` strategy. JSON files shall be serialized per RFC 8785 (JSON Canonicalization Scheme). YAML and TOML files declared as `merge = "deep"` shall be converted to RFC 8785 canonical JSON before hashing.
  3. Append the effective parameter set as a canonical JSON object (RFC 8785) at the synthetic path `.aenv/parameters.json` with content being the resolved parameter map.
  4. Sort the pair set by `relative_path`, using byte-wise lexicographic ordering on the UTF-8 encoding of the path. Path comparison shall not be locale-aware and shall not apply Unicode normalization.
  5. Construct the hash input by concatenating, for each pair in sorted order: a 4-byte big-endian length prefix of the path, the path bytes, an 8-byte big-endian length prefix of the content, and the content bytes.
  6. Prepend a single algorithm-version byte (`0x01`) to the hash input before SHA-256 computation.
- **R-85 (Ubiquitous)** The hash shall be exposed in `--json` output as a string of the form `sha256-v1:<hex>` where `<hex>` is the lowercase hexadecimal SHA-256 digest.
- **R-86 (Ubiquitous)** The hash shall not incorporate any of the following: file modification times, file permissions, symlink targets, manifest file formatting (only the resolved parameter map is hashed, not the manifest text itself), registry git state, shadow-set metadata, or any data outside the resolved file set and effective parameter map.
- **R-87 (Ubiquitous)** Any change to the canonicalization algorithm in R-84 shall constitute a breaking change. Such a change shall be introduced under a new algorithm-version byte (`0x02`, etc.) and a new hash prefix (`sha256-v2:`); for at least one major release after introduction, the system shall emit both the old and new hash strings in `--json` output to allow consumers to migrate.

---

## 6. Open questions

- **Skill scaffolding templates.** `aenv skill new` produces a SKILL.md with adapter-appropriate frontmatter. Should the body be a minimal stub or a richer template? Recommend minimal stub for v1 and let users override with `--template` later.
- **Per-session vs per-directory.** Should a namespace be activatable for a single shell session without writing `.aenv`? Probably yes (`aenv shell <name>`) but out of scope for v1.
- **Secrets in MCP configs.** Some `.mcp.json` files contain tokens. Recommend documenting that namespaces should not contain secrets and providing a `${ENV_VAR}` interpolation syntax in v2.
- **Tool restart signaling.** Agents that cache configs at startup won't see namespace changes until restart. Out of scope to solve; document clearly.
- **Windows symlinks.** Copy-mode fallback is specified; needs testing.
- **Skill registry format.** R-16 mentions a "configured skill registry" as one of the source types for imported skills. The registry format is not specified in v1; treat as a forward-compatibility hook.
- **Parameter typing extensions.** R-66 supports string/int/bool/list-of-string. Should v1 support nested tables, maps, or richer types? Recommend deferring; revisit when adapter projection needs arise.
- **Policy DSL extensibility.** R-72 lists a starter set of policy keys. The mechanism for adapters to register custom policy keys is not specified in v1.

## 7. Success criteria

- A user can define three namespaces (`base`, `python-strict`, `rust-strict`), pin two different projects to compositions of them, `cd` between projects, and observe the correct files materialized in each — with zero manual file copying.
- A team can share a namespace via git such that a new contributor running `aenv install` in a freshly cloned project gets the same harness as everyone else.
- A user can swap between two namespaces on the same project and run the same coding task twice, with the only variable being the harness configuration.
- A downstream script can drive `aenv` end-to-end — activate, query state, deactivate — using only `--json` output and documented exit codes, with no parsing of human-readable text. All artifact references in that output carry qualified identities.
- Two users on different machines who activate the same namespace at the same registry commit observe the same content hash.
- A user can scaffold a new skill into a namespace via `aenv skill new`, import an external skill via `aenv skill import`, and observe both materialized correctly at activation with their qualified provenance recorded in state.
- A user creating a new namespace can author it skill-heavy and CLAUDE.md-light, receive no soft-limit warnings, and observe `aenv doctor` reporting a clean bill of health.
- A user can run `aenv which .claude/skills/write-tests/SKILL.md` and receive a reply naming both the providing namespace (`detailed-execution::write-tests`) and any shadowed namespace (`base::write-tests`).
- A user can set a parameter via `aenv set detailed-execution.default_model claude-opus-4.5`, observe the adapter project it into `.claude/settings.json`, and read the effective value back via `aenv get`.

## 8. Out of scope for v1

- GUI or editor integrations.
- Behavioral evaluation or A/B comparison of agents under different namespaces. A separate project may consume `aenv` for this purpose via its JSON interfaces and exit codes; `aenv` itself does not perform evaluation.
- Per-file ACLs or fine-grained permissions on namespaces.
- Encrypted namespace contents.
- Runtime namespace isolation (multiple namespaces simultaneously active in the same project, agent-side namespace awareness, namespace-qualified artifact invocation by the user inside an agent session).
- Cross-namespace artifact references (e.g., a `detailed-execution` rule referring directly to `analyst::trace-callgraph`). The only composition primitive is `extends`.
