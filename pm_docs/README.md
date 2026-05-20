# aenv — Document Bundle

Three documents specifying `aenv`, a virtual environment system for AI coding harness configs (CLAUDE.md, .cursorrules, .mcp.json, skills, agents, etc.) — like Python's `venv`, but for the rules and configurations that shape how AI coding agents behave.

## The documents

**[aenv-prd.md](./aenv-prd.md)** — Product requirements in EARS format. The public contract: what the system does, what guarantees it makes. Read this first. v0.3 introduces namespace identity, parameters, and policies as first-class concepts.

**[aenv-functional-spec.md](./aenv-functional-spec.md)** — How users actually interact with `aenv` on a Tuesday. Walks through three example harnesses (`experiments`, `detailed-execution`, `analyst`) and a full set of user journeys, including the scriptability surface that downstream tools will consume. Maps to the PRD's requirements.

**[aenv-engineering.md](./aenv-engineering.md)** — Internal engineering decisions: Rust implementation, crate selection, error/exit-code strategy, the `Filesystem` trait for testability, namespace identity in the internal model, the rename-atomicity pitfall, testing approach, the `git` shell-out dependency. Lives outside the PRD because it can evolve as long as the public contracts hold.

## Status

- PRD: v0.3 draft (namespaces, parameters, policies)
- Functional spec: v0.3 draft
- Engineering doc: v0.2 draft

All three are working drafts. Versions move together when public contracts change.

## Vocabulary

The primary unit of organization is a **namespace** — a named, directory-backed bundle of harness config files, skills, agents, parameters, and policies. The term *env* is retained as a deprecated alias in CLI command names (`aenv use`, `aenv list`) and the `AENV_HOME` environment variable for brevity, but the documentation and structured output use *namespace* as the canonical term.

Artifacts inside a namespace are addressable by qualified name: `namespace::short_name` (e.g. `detailed-execution::write-tests`). Parameters are addressable as `namespace.parameter` (e.g. `detailed-execution.default_model`). The `::` and `.` separators are visually distinct on purpose — one is about ownership of artifacts, the other is about typed configuration values.

## Reading order

For a reviewer new to the project: PRD → functional spec → engineering doc.

For someone implementing: engineering doc → PRD requirements → functional spec for behavior verification.

For someone building a downstream consumer (e.g. a harness evaluation tool): PRD §5.14, §5.15, §5.16 (scriptability, hash, parameters), then functional spec §7.
