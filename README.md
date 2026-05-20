# aenv — Virtual environments for AI coding harness configs

`aenv` is a Rust CLI for managing named, composable, version-controlled bundles of AI-coding-agent configuration (`CLAUDE.md`, `.cursorrules`, `.mcp.json`, skills, agents, slash commands, MCP entries). Think Python's `venv`, but for the rules and configurations that shape how AI coding agents behave.

> **Status:** Early development. The CLI does not yet do anything useful — Phase 0 (project skeleton) is the most recent milestone. See [`tasks/todo.md`](./tasks/todo.md) for the full roadmap.

## Reading order

- **[`pm_docs/aenv-prd.md`](./pm_docs/aenv-prd.md)** — Product requirements in EARS format. The public contract.
- **[`pm_docs/aenv-functional-spec.md`](./pm_docs/aenv-functional-spec.md)** — How users interact with `aenv`. Three example harnesses, twelve user journeys.
- **[`pm_docs/aenv-engineering.md`](./pm_docs/aenv-engineering.md)** — Internal implementation decisions (Rust, crate selection, error/exit-code strategy, `Filesystem` trait, namespace identity model).
- **[`tasks/todo.md`](./tasks/todo.md)** — Implementation roadmap with phase mapping back to PRD requirements.

## Building

```bash
cargo build --workspace
cargo test --workspace
```

Requires Rust stable 1.79 or later.

## License

Dual-licensed under MIT or Apache 2.0.
