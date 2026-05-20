# aenv Adapter Research: Configuration & Harness Systems Across Six AI Coding Agents

## TL;DR
- **Every tool has converged on the same dual surface** — a markdown "rules/memory" file (CLAUDE.md, .cursor/rules/*.mdc, .windsurf/rules/*.md, .clinerules/*, CONVENTIONS.md, .continue/rules/*.md) plus a structured config (settings.json, .aider.conf.yml, config.yaml, cline_mcp_settings.json) — and **most also support `.mcp.json`-style MCP server configs and per-tool skill/agent/command directories**, so aenv should model adapters around three orthogonal surfaces (instructions, structured settings, MCP) rather than a single file.
- **Reload behavior is the single biggest hazard for aenv runtime correctness**: Claude Code requires session restart for CLAUDE.md/settings.json edits (with one exception — root CLAUDE.md is re-read after `/compact`); Cline hot-reloads MCP for some fields and requires window reload for others; Windsurf and Cursor commonly need a window reload when rules are *added* (not just edited); only Continue documents explicit hot-reload on save. aenv must treat "switch" as "swap files + signal/restart agent process" per adapter.
- **Stay with the file-symlink/copy model, not a daemon**: precedence is heterogeneous (override vs. concatenate vs. dedup-and-merge vs. deny-wins), so aenv should expose a per-adapter `merge_strategy` and keep "global vs project vs local" semantics intact rather than collapsing them into one virtual file.

---

## Key Findings

1. **Markdown-with-YAML-frontmatter has won as the rule format.** Cursor (.mdc), Windsurf (.windsurf/rules/*.md), Cline (.clinerules/*.md), Continue (.continue/rules/*.md), and Claude Code skills (SKILL.md) all use the same shape: YAML frontmatter (`description`, `globs`/`trigger`, `alwaysApply`) over Markdown body. Only Aider deviates (plain markdown loaded as a read-only chat file). aenv can ship one normalizer and write to each tool's frontmatter dialect.

2. **AGENTS.md is the emerging cross-tool standard** (governed by the Linux Foundation's Agentic AI Foundation). Cursor, Windsurf, Cline (via the community `clinerules` repo), and Aider read AGENTS.md; Claude Code does not yet (community uses symlinks). aenv should treat AGENTS.md as a "fan-out target" several adapters share.

3. **Three-to-five scope tiers are nearly universal**: managed/enterprise (Claude Code, Windsurf system rules) → user/global (`~/.claude/`, `~/.cursor/`, `~/.codeium/windsurf/`, `~/.continue/`, `~/Documents/Cline/`) → project (`./.claude/`, `./.cursor/`, `./.windsurf/`, `./.clinerules/`, `./.continue/`) → local-personal (`.claude/settings.local.json`, `CLAUDE.local.md`).

4. **MCP config locations are tool-specific and not interoperable out of the box**, but Continue can ingest Cursor/Claude/Cline JSON verbatim into `.continue/mcpServers/`. The JSON schemas are essentially the same `{ "mcpServers": { name: { command, args, env, url, headers } } }` shape, so aenv can use one canonical schema and emit tool-specific wrappers.

5. **Reload semantics are split into three classes**:
   - **Hot-reload** (Continue's documented behavior: *"When you save a config file from the IDE, Continue will automatically refresh to take into account your changes"* — docs.continue.dev).
   - **Process-restart required** (Claude Code settings.json/CLAUDE.md, per anthropics/claude-code issue #15858: *"changes aren't picked up by running sessions. Users must restart each session to apply updates"*; Aider session lifetime).
   - **Field-dependent / window-reload-recommended** (Cline MCP: timeout edits are in-memory, command/args edits trigger `restartConnection`; Cursor and Windsurf when *adding* rules).

6. **Precedence rules are NOT uniform**. Claude Code concatenates CLAUDE.md files (no override; per docs: "All discovered files are concatenated into context rather than overriding each other… when there's a conflict, Claude may pick one arbitrarily"), but for settings.json: scalars override (more specific wins), arrays concatenate-and-dedupe, deny rules always win over allow. Cursor and Windsurf treat workspace as overriding global on conflict. Cline merges global+workspace with workspace winning on conflict. aenv must record this per-key.

---

## Details — Per-Tool Adapter Blueprints

### 1. Claude Code (Anthropic CLI)

**Config file locations** (loaded in this order, later wins for scalars; arrays merge):
- Enterprise managed: `/etc/claude-code/managed-settings.json` (Linux), `/Library/Application Support/ClaudeCode/managed-settings.json` (macOS), `C:\ProgramData\ClaudeCode\managed-settings.json` (Windows). Cannot be overridden.
- User: `~/.claude/settings.json`, `~/.claude/CLAUDE.md`, `~/.claude.json` (MCP user-scope).
- Project shared: `./.claude/settings.json`, `./CLAUDE.md` (or `./.claude/CLAUDE.md`), `./.mcp.json`.
- Project local: `./.claude/settings.local.json`, `./CLAUDE.local.md` (auto-gitignored).
- Session: `--settings <path>` CLI flag.
- `CLAUDE_CONFIG_DIR` env var relocates `~/.claude`.

**Formats**: `settings.json` (JSON), `CLAUDE.md` (Markdown with `@path/to/file` imports — per code.claude.com/docs/en/memory: *"CLAUDE.md files can import additional files using `@path/to/import` syntax. Imported files are expanded and loaded into context at launch alongside the CLAUDE.md that references them… Imported files can recursively import other files, with a maximum depth of five hops."*), `.mcp.json` (standard `mcpServers` JSON).

**Fields**: `permissions.{allow,deny,ask,defaultMode,additionalDirectories,disableBypassPermissionsMode}`, `env`, `model`, `hooks`, `mcpServers`, `enabledPlugins`, `attribution`, `sandbox.{filesystem,network}`, `policyHelper` (managed-only), `apiProvider`, `cleanupPeriodDays`, `alwaysThinkingEnabled`, `teammateMode`. As of v2.1.139, Claude Code exposes 60+ settings and 180+ environment variables. CLAUDE.md is plain markdown with conventional sections (Build & Test, Standards, Architecture).

**Multiple files / composition**: CLAUDE.md walks upward from cwd to git root concatenating each level; `CLAUDE.local.md` is appended after `CLAUDE.md` within each directory; `@path` imports recursively up to depth 5.

**Skills / agents / commands**:
- `./.claude/skills/<name>/SKILL.md` (+ supporting scripts) — model-invoked, auto-discovered at startup via metadata frontmatter; commands have been unified with skills (a file at `.claude/commands/deploy.md` and a skill at `.claude/skills/deploy/SKILL.md` both create `/deploy`).
- `./.claude/agents/<name>.md` (project) or `~/.claude/agents/<name>.md` (user) — Markdown with YAML frontmatter (`name`, `description`, `prompt`, `tools`, `disallowedTools`, `model`, `permissionMode`, `mcpServers`, `hooks`, `maxTurns`, `skills`, `memory`, `effort`, `background`, `isolation`, `color`).
- `./.claude/hooks/*.sh`, registered under `hooks` key in settings.json.
- `./.claude/plugins/` for installed plugin manifests.

**MCP**: project-scope `./.mcp.json`, user-scope merged into `~/.claude.json`, managed `managed-mcp.json`. Plus three CLI scopes via `claude mcp add --scope {local,project,user}`. Tool Search caps idle MCP tool descriptions at 10% of context.

**Caching / reload**: **Restart required.** anthropics/claude-code issue #15858: *"When iterating on Claude Code configuration (CLAUDE.md, settings.json), changes aren't picked up by running sessions. Users must restart each session to apply updates."* Exception: root CLAUDE.md is re-read from disk on `/compact`. Subdirectory CLAUDE.md files reload the next time Claude reads a file there.

**Validation**: JSON schema exposed; `/status` reports invalid JSON and shows which scope supplied each setting; managed `policyHelper` only honored from managed scope. Five timestamped backups retained automatically per config file. Invalid keys in settings.json are silently ignored.

**Precedence**: Single values: managed > session (`--settings`) > project local > project shared > user. Arrays (e.g., `permissions.allow`, `sandbox.filesystem.allowWrite`): concatenated and deduplicated across scopes. **Deny rules always win across all scopes.**

**aenv adapter blueprint**:
```
adapter: claude-code
files:
  - {role: instructions, path: ./CLAUDE.md, format: markdown+@imports, merge: concat-up-tree}
  - {role: instructions-local, path: ./CLAUDE.local.md, gitignored: true}
  - {role: settings, path: ./.claude/settings.json, merge: override-scalar+merge-array}
  - {role: settings-local, path: ./.claude/settings.local.json, gitignored: true}
  - {role: mcp, path: ./.mcp.json}
  - {role: skills, path: ./.claude/skills/, file: SKILL.md}
  - {role: agents, path: ./.claude/agents/*.md}
  - {role: commands, path: ./.claude/commands/*.md}
  - {role: hooks, path: ./.claude/hooks/*}
reload_strategy: restart-session
backup: built-in (5 timestamped copies)
gotchas:
  - deny rules in settings.local.json CAN'T override managed deny
  - import depth max 5
  - mid-session edits silently ignored until restart
```

### 2. Cursor

**Config file locations**:
- Project: `./.cursor/rules/<name>.mdc` (or `./.cursor/rules/<name>/RULE.md` per Cursor 2.2+ — *"As of 2.2, .mdc cursor rules will remain functional however all new rules will now be created as folders in .cursor/rules. This is to improve the readability and maintainability of rules"* — Cursor 2.2 release notes), `./.cursor/commands/<name>.md`, `./.cursor/mcp.json`. Legacy `./.cursorrules` still works but is deprecated.
- User: `~/.cursor/rules/`, `~/.cursor/commands/`, `~/.cursor/mcp.json`.
- Team: Cursor.com dashboard → Team Rules (Business/Enterprise only).
- `AGENTS.md` at project root or subdirectories is read alongside rules.

**Formats**: `.mdc` = YAML frontmatter (`description`, `globs`, `alwaysApply`) + Markdown body, optionally with `@filename.ts` file references. From Cursor 2.2, new rules are folders containing `RULE.md`. `mcp.json` is the standard `mcpServers` JSON.

**Rule activation types** (set via frontmatter):
- `alwaysApply: true` → Always rule.
- `globs: "src/**/*.tsx"` → Auto Attached when matching files are referenced.
- `description: "..."` + `alwaysApply: false` (no globs) → Agent Requested (model decides).
- No frontmatter activation → Manual via `@rule-name`.

**Multiple files / composition**: Recursive — Cursor reads all `.cursor/rules/` directories from the workspace tree, with nested rules auto-attaching when files in their directory are referenced. No declared precedence between conflicting rules; community notes: *"Cursor will follow the most recently loaded rule or produce inconsistent results."*

**Skills / agents / commands**:
- `./.cursor/commands/<name>.md` → slash-command prompt template (introduced Cursor 1.6).
- Cursor 2.4 (January 2026) added Agent Skills: *"Define skills in SKILL.md files, which can include custom commands, scripts, and instructions for specializing the agent's capabilities based on the task at hand."* (cursor.com/changelog/2-4).
- Custom Modes (Cursor Settings → Chat → Custom Modes, in beta) — define tool combinations and instructions; not file-based, stored in user settings.
- Plan / Ask / Agent modes are built-in.

**MCP**: `./.cursor/mcp.json` (project) and `~/.cursor/mcp.json` (global). Native HTTP transport supported. Cursor 2.4 changelog: *"MCP server definitions and tools now live as JSON files in .cursor. Agents discover and load MCPs only when needed, reducing token usage and keeping the context focused."*

**Caching / reload**: New `.mdc` files generally apply without restart, but **adding/renaming rule files often requires a window reload** in practice (community reports). MCP server config additions show a green-dot connection status; the Cursor 2.x cycle added per-server `/mcp enable|disable` slash commands without restart.

**Validation**: Glob syntax is strict — `src/**/*.tsx` matches subdirectories; `src/*.tsx` does not. Description must be non-empty for Agent Requested rules to fire.

**Precedence**: Team Rules > Project Rules > User Rules. `.cursorrules` legacy still loaded but warns deprecation. AGENTS.md is read at project root and subdirectories.

**aenv adapter blueprint**:
```
adapter: cursor
files:
  - {role: instructions, path: ./AGENTS.md, optional: true}
  - {role: rules, path: ./.cursor/rules/*.mdc, format: mdc, merge: concat-by-filename}
  - {role: commands, path: ./.cursor/commands/*.md}
  - {role: mcp, path: ./.cursor/mcp.json}
  - {role: legacy, path: ./.cursorrules, deprecated: true}
reload_strategy: hot-for-edits / restart-for-additions
gotchas:
  - team rules live on Cursor.com dashboard, not files (out of aenv's reach)
  - conflicting rules → undefined behavior, no documented precedence
  - .cursor/rules folder-with-RULE.md vs single .mdc — both formats coexist
```

### 3. Aider

**Config file locations** (loaded in order, later wins):
- `~/.aider.conf.yml`
- Git repo root `.aider.conf.yml`
- Current working directory `.aider.conf.yml`
- Or single file via `--config <path>`.
Plus parallel files for model settings (`.aider.model.settings.yml`) and metadata (`.aider.model.metadata.json`) with the same lookup order. Environment variables `AIDER_*` and a `.env` file in git root also feed config.

**Formats**: YAML for `.aider.conf.yml` and `.aider.model.settings.yml`; JSON for `.aider.model.metadata.json`; Markdown for `CONVENTIONS.md` (or any name) loaded as a read-only file. Note: *"You can only put OpenAI and Anthropic API keys in the YAML config file. Keys for all APIs can be stored in a .env file."*

**Fields**: `model`, `openai-api-key`, `anthropic-api-key`, `openai-api-base`, `read` (list of read-only files), `file` (list of editable files), `architect`/`sonnet`/`opus` shortcuts, `alias`, `chat-language`, `commit-language`, `yes-always`, `auto-commits`, `dirty-commits`, `lint-cmd`, `test-cmd`, `notifications`, `edit-format`, `reasoning-effort`, `thinking-tokens`. Lists accept either bullet or `[a, b, c]` style.

**Multiple files / composition**: No native include syntax. Config files are loaded sequentially (home → repo root → cwd) and later files **override** earlier ones. The `read:` list accepts multiple paths for conventions composition; global conventions are still painful (per open issue #3433 — relative paths resolve against cwd, not the file containing the directive).

**Skills / agents / commands**: **None.** Aider has no skill/agent/subagent system. Slash commands are built-in (`/add`, `/read`, `/architect`, `/code`, `/ask`, `/help`, `/reset`, `/clear`, `/editor`). A `load:` config option runs a file of `/commands` on launch.

**MCP**: **No native MCP client support** as of mid-2026.

**Caching / reload**: Aider re-reads files in the chat on each message — Aider FAQ: *"Aider always reads the latest copy of files from the file system when you send each message."* Config itself is loaded once at startup; changing `.aider.conf.yml` requires restarting aider. `/reset` drops files from chat — re-add `CONVENTIONS.md` after a reset (issue #3060).

**Validation**: No published JSON Schema. Unknown keys cause CLI errors.

**Precedence**: Cwd > repo root > home for config files (later wins). CLI flags > env vars > config file.

**aenv adapter blueprint**:
```
adapter: aider
files:
  - {role: settings, path: ./.aider.conf.yml, format: yaml, merge: override}
  - {role: model-settings, path: ./.aider.model.settings.yml, format: yaml}
  - {role: model-metadata, path: ./.aider.model.metadata.json, format: json}
  - {role: env, path: ./.env}
  - {role: conventions, path: ./CONVENTIONS.md, registered_via: read: in conf.yml}
reload_strategy: restart-process
gotchas:
  - no MCP, no skills, no agents — aenv adapter ONLY swaps yml/md
  - "read:" paths resolve relative to cwd, not the config file (issue #3433)
  - keep API keys in .env, not .aider.conf.yml (config file limited to OpenAI/Anthropic)
```

### 4. Cline (VS Code extension)

**Config file locations**:
- Project: `./.clinerules/` (preferred; directory of `.md`/`.txt` files) or single `./.clinerules` file. Plus `./.clinerules/workflows/*.md`.
- Global (per Cline CLI 1.x docs): `~/.cline/data/settings/{providers.json, global-settings.json, cline_mcp_settings.json}`, `~/.cline/data/workflows/`, `~/.cline/rules/`, `~/.cline/hooks/`, `~/.cline/skills/`, `~/.cline/agents/`, `~/.cline/plugins/`, `~/.cline/cron/`. Additional global paths: `~/Documents/Cline/{Rules,Hooks,Plugins,Workflows}/` for backward compat.
- VS Code extension MCP storage (legacy path still in use): `~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` (macOS); `%APPDATA%/Code/User/globalStorage/...` (Windows); `~/.config/Code/User/globalStorage/...` (Linux).
- `./.clineignore` controls file access (gitignore syntax).

**Formats**: Markdown for rules and workflows; JSON for `cline_mcp_settings.json`, `providers.json`, `global-settings.json`. Rule files support YAML frontmatter for conditional rules: `--- paths: ["docs/**", "**/*.md"] description: "Docs rules" ---`. Toggle UI uses the frontmatter `description` as the label.

**Fields**:
- Rules frontmatter: `paths` (glob list), `description`.
- `cline_mcp_settings.json`: standard `mcpServers` JSON with `disabled`, `autoApprove`, `alwaysAllow` extensions.

**Multiple files / composition**: Cline loads all `.md`/`.txt` files in `.clinerules/` alphabetically (prefix with `01-`, `02-` to control order). Global + workspace rules merge; workspace wins on conflict. Cline v3.13 added a toggle UI in the Rules popover; toggle state doesn't persist across VS Code sessions.

**Skills / agents / commands**:
- `~/.cline/skills/` and `./.cline/skills/` (CLI era; the original VS Code extension didn't have skills directories before the CLI unification).
- `~/.cline/agents/` and `./.cline/agents/` for agent definitions.
- Workflows: `./.clinerules/workflows/*.md` (workspace) and `~/Documents/Cline/Workflows/*.md` or `~/.cline/data/workflows/` (global). Invoked with `/<filename>.md`. Local workflows take precedence over global ones with the same name.
- Slash commands: `/newrule`, `/newtask`, `/<workflow>.md`.
- Hooks: `~/.cline/hooks/` (global), `./.cline/hooks/` (project).

**MCP**: Settings JSON at the global-storage path above (per-machine, NOT in the project). Long-standing community feature request (Discussion #2418) for per-project MCP — Cline does not natively support `.mcp.json` at project scope as of 2026; users typically swap the global file or use git to manage it. Cline also has a built-in MCP marketplace ("Extensions" button) for one-click install.

**Caching / reload**:
- Rules: hot-reloadable when toggled in UI; the rules popover re-reads files.
- MCP config: Cline watches `cline_mcp_settings.json` and per its architecture has `configsRequireRestart()` logic distinguishing connection-affecting fields (command/args trigger `restartConnection`) from in-memory fields (timeout). Community guidance widely says "restart VS Code to let it take effect" for new server additions — treat as needing a reload in adapter design.
- `.clinerules` re-read at task start.

**Validation**: No published JSON schema. Toggle UI shows which rules are active; if a rule has no description, filename is used as label. Frontmatter glob patterns are matched against working files; invalid YAML breaks the file silently.

**Precedence**: Workspace `.clinerules` overrides global Custom Instructions; workspace rules win over global rules on conflict.

**aenv adapter blueprint**:
```
adapter: cline
files:
  - {role: rules-project, path: ./.clinerules/*.md, format: md+yaml-fm, merge: concat-alpha}
  - {role: rules-global, path: ~/.cline/rules/*.md or ~/Documents/Cline/Rules/*.md}
  - {role: workflows-project, path: ./.clinerules/workflows/*.md}
  - {role: workflows-global, path: ~/.cline/data/workflows/*.md}
  - {role: mcp, path: ~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json}
  - {role: providers, path: ~/.cline/data/settings/providers.json}
  - {role: clineignore, path: ./.clineignore}
reload_strategy: reload-vscode-window-for-mcp; hot-toggle-for-rules
gotchas:
  - MCP is global-only (per-project unsupported); aenv must swap the global file or symlink
  - VS Code globalStorage path uses extension-id "saoudrizwan.claude-dev" (Cline's legacy id)
  - Toggle UI state doesn't persist between sessions
  - Two CLI namespaces: ~/.cline/ and ~/Documents/Cline/ both supported
```

### 5. Continue (VS Code / JetBrains extension)

**Config file locations**:
- Global: `~/.continue/config.yaml` (preferred) or `~/.continue/config.json` (deprecated). Plus `~/.continue/config.ts` for TypeScript programmatic extension. Workspace-level `.continuerc.json` overrides global with `mergeBehavior: "merge" | "overwrite"`.
- Project / workspace organization:
  - `./.continue/rules/*.md` — rules.
  - `./.continue/mcpServers/*.yaml` (or `.json`) — MCP servers (note plural "Servers").
  - `./.continue/models/*.yaml` — local model defs.
  - `./.continue/prompts/*.prompt` — custom prompts.
  - `./.continue/.env` and `<workspace>/.env` — secrets.

**Formats**: YAML (`config.yaml`) is preferred; JSON (`config.json`) is the deprecated legacy schema. Rules are Markdown with YAML frontmatter (`name`, `globs`, `regex`, `description`, `alwaysApply`). MCP server files are YAML (with `name`, `version`, `schema: v1`, `mcpServers: [...]`).

**Fields** (config.yaml schema v1): `name`, `version`, `schema`, `models[]` (with `provider`, `model`, `roles[]`, `defaultCompletionOptions`, `autocompleteOptions`, `embedOptions`, `chatOptions`, `promptTemplates`, `capabilities`, `requestOptions`), `context[]`, `rules[]`, `prompts[]`, `docs[]`, `mcpServers[]`, `data[]`. Hub imports use `uses: owner/item-name`. Secrets via `${{ secrets.NAME }}` mustache syntax. The deprecated `config.json` has `systemMessage` (now `rules`), `contextProviders` (now `context`), `customCommands` (now `prompts`), and `experimental.modelContextProtocolServers` (now `mcpServers`).

**Multiple files / composition**: Files in `.continue/rules/`, `.continue/mcpServers/`, `.continue/models/` are auto-discovered and concatenated. Rules load order: Hub assistant rules → Local config rules → Workspace `.continue/rules/`. Continue can also ingest JSON MCP files from other tools dropped into `.continue/mcpServers/` (documented).

**Skills / agents / commands**:
- Prompts (`.continue/prompts/<name>.prompt` or in `config.yaml`) become `/<name>` slash commands.
- "Agents" in Continue terminology = the whole config.yaml (an assistant defined by models + rules + tools).
- No subagents, no skills (in the Claude Code sense).
- The deprecated `slashCommands` array in config.json/ts has been replaced by prompts.

**MCP**: `.continue/mcpServers/*.yaml` or `*.json`. Transports: `stdio`, `sse`, `streamable-http`. Only works in Agent mode. Cross-tool ingestion: official docs confirm *"If you're coming from another tool that uses JSON MCP format configuration files (like Claude Desktop, Cursor, or Cline), you can copy those JSON config files directly into your .continue/mcpServers/ directory."*

**Caching / reload**: **Hot-reload on save** — docs.continue.dev/customize/deep-dives/configuration: *"When you save a config file from the IDE, Continue will automatically refresh to take into account your changes. A config file is automatically created the first time you use Continue, and always automatically generated with default values if it doesn't exist."* This is the only one of the six tools with documented auto-reload.

**Validation**: JSON schema published at `extensions/vscode/config_schema.json` (used by VS Code/JetBrains for autocomplete). Rules issue #6905 reports rules being silently ignored in some setups — known bug class.

**Precedence**: Global config.yaml → workspace `.continuerc.json` (merge or overwrite per setting). Hub-imported rules → local config rules → workspace `.continue/rules/`. Deprecated `config.json` settings auto-migrate to user settings on first load.

**aenv adapter blueprint**:
```
adapter: continue
files:
  - {role: settings, path: ~/.continue/config.yaml, format: yaml, schema: v1}
  - {role: settings-workspace, path: ./.continuerc.json, merge: per-mergeBehavior-field}
  - {role: rules, path: ./.continue/rules/*.md, format: md+yaml-fm}
  - {role: prompts, path: ./.continue/prompts/*.prompt}
  - {role: mcp, path: ./.continue/mcpServers/*.{yaml,json}}
  - {role: models, path: ./.continue/models/*.yaml}
  - {role: secrets, path: ./.continue/.env or ./.env}
  - {role: extension, path: ~/.continue/config.ts, deprecated-but-supported: true}
reload_strategy: auto-hot-reload-on-save
gotchas:
  - config.json is deprecated; auto-migrates but new features only in config.yaml
  - cross-tool MCP JSON import works (drop file in mcpServers/)
  - secrets resolve in order: workspace .env → ~/.continue/.env → Hub secrets
```

### 6. Windsurf (Cognition, formerly Codeium)

**Config file locations**:
- Project: `./.windsurf/rules/*.md` (preferred), legacy `./.windsurfrules` (still supported), `./.windsurf/workflows/*.md`, `./.windsurf/skills/<name>/SKILL.md`, `./AGENTS.md` (read at project root and subdirectories).
- Global: `~/.codeium/windsurf/memories/global_rules.md`, `~/.codeium/windsurf/mcp_config.json`.
- System (enterprise managed): OS-specific paths under `/Library/Application Support/Codeium/...` (macOS), `%PROGRAMDATA%\Codeium\...` (Windows), `/etc/codeium/...` (Linux) — system-level rules merged with workspace and global rules.
- Cross-agent fallback dirs: `./.agents/skills/`, `~/.agents/skills/` (Windsurf reads these for portability).

**Formats**: Plain Markdown for rules; `global_rules.md` and root-level `AGENTS.md` files do NOT use frontmatter (always on). Workspace rules use YAML frontmatter with `trigger: {always_on | manual | glob | model_decision}`, `description`, `globs`. SKILL.md uses standard frontmatter (`name`, `description`). Workflows use frontmatter (`name`, `description`, `auto_execute_steps`). MCP config is standard `mcpServers` JSON.

**Fields**: Rule frontmatter trigger modes:
- `always_on` — included every prompt.
- `manual` — only when `@rule-name` is mentioned.
- `glob` — included when files matching globs are touched.
- `model_decision` — Cascade decides based on `description`.

**Multiple files / composition**: Windsurf auto-discovers `.windsurf/rules/` directories within the workspace and (for git repos) walks up to the git root. Multi-workspace: rules deduplicated, displayed with shortest relative path.

**Character limits** (these are real and trip up aenv): Per-rule limit — the official Windsurf docs state: *"global_rules.md and .windsurfrules are limited to 6000 characters each. Any content above 6000 characters will be truncated and Cascade will not be aware of them."* The widely-cited 12,000-character total cap on combined active rules is **community-reported, not in official Windsurf docs** — aenv should warn at 6k per file (documented) and treat the 12k total as a soft heuristic.

**Skills / agents / commands**:
- Skills: `./.windsurf/skills/<name>/SKILL.md` (workspace) and `~/.windsurf/skills/<name>/SKILL.md` (global). Same SKILL.md format as Claude Code.
- Workflows: `./.windsurf/workflows/*.md` invoked with `/<workflow-name>`. Workflows orchestrate multi-step procedures.
- Memories: auto-generated by Cascade (per-user, not file-based, managed via "Manage memories" UI).
- No subagent feature.

**MCP**: `~/.codeium/windsurf/mcp_config.json` is the canonical location. Windsurf uses `mcp-remote` bridge for HTTP servers (like Claude Desktop). The Cascade panel has a hammer icon → "Configure" → "View raw config". Some setups use `serverUrl` and `headers` keys (per remoet.dev guide).

**Caching / reload**: Restart commonly required when adding new rules ("Restart Windsurf if rules were recently added" is the most-cited fix when Cascade ignores a rule). Rules already loaded are picked up between turns. MCP changes require a Cascade refresh (click Refresh in MCP panel) but generally not a full restart.

**Validation**: No public JSON schema. Character limits are enforced silently (no error, just truncation/drop). Glob patterns must match working files.

**Precedence**: System (enterprise) → Global (`global_rules.md`) → Workspace (`.windsurf/rules/`). Workspace overrides global on conflict. AGENTS.md is read alongside; "closest AGENTS.md to the edited file wins."

**aenv adapter blueprint**:
```
adapter: windsurf
files:
  - {role: instructions, path: ./AGENTS.md, optional: true, scoped_by_subdir: true}
  - {role: rules-workspace, path: ./.windsurf/rules/*.md, format: md+yaml-fm}
  - {role: rules-global, path: ~/.codeium/windsurf/memories/global_rules.md}
  - {role: rules-system, path: /Library/Application Support/Codeium/.../rules/*.md, enterprise-only: true}
  - {role: workflows, path: ./.windsurf/workflows/*.md}
  - {role: skills, path: ./.windsurf/skills/*/SKILL.md}
  - {role: mcp, path: ~/.codeium/windsurf/mcp_config.json}
  - {role: legacy, path: ./.windsurfrules, deprecated: true}
reload_strategy: restart-for-new-rules; refresh-for-mcp
gotchas:
  - hard char limit: 6k per file (official); 12k total active (community-reported) — silent truncation/drop
  - global_rules.md and root AGENTS.md ignore frontmatter (always on)
  - MCP location is under ~/.codeium/, NOT ~/.windsurf/
  - cross-agent skill discovery looks in .agents/skills/ too
```

---

## Comparative Matrix

| Dimension | Claude Code | Cursor | Aider | Cline | Continue | Windsurf |
|---|---|---|---|---|---|---|
| **Primary rules file** | `CLAUDE.md` | `.cursor/rules/*.mdc` | `CONVENTIONS.md` (via `read:`) | `.clinerules/*.md` | `.continue/rules/*.md` | `.windsurf/rules/*.md` |
| **Legacy rules file** | n/a | `.cursorrules` (deprecated) | n/a | single `.clinerules` file | `config.json` `systemMessage` | `.windsurfrules` |
| **Cross-tool standard** | not yet | reads `AGENTS.md` | reads `AGENTS.md` | reads `AGENTS.md` | rules only | reads `AGENTS.md` |
| **Structured settings** | `settings.json` (JSON) | settings stored in app; rules in MDC | `.aider.conf.yml` (YAML) | `cline_mcp_settings.json` + providers.json | `config.yaml` (YAML) | none beyond rules |
| **Format** | MD + YAML frontmatter for skills/agents; JSON for settings | MDC (YAML FM + MD) | YAML, JSON, MD, .env | MD + YAML FM; JSON for MCP | YAML (preferred), JSON (legacy), MD | MD + YAML FM |
| **Project scope** | `./.claude/`, `./CLAUDE.md`, `./.mcp.json` | `./.cursor/` | `./.aider.conf.yml`, `./CONVENTIONS.md` | `./.clinerules/` | `./.continue/` | `./.windsurf/`, `./AGENTS.md` |
| **User scope** | `~/.claude/`, `~/.claude.json` | `~/.cursor/` | `~/.aider.conf.yml` | `~/.cline/`, `~/Documents/Cline/`, VS Code globalStorage | `~/.continue/` | `~/.codeium/windsurf/` |
| **Enterprise/managed** | yes — `managed-settings.json` + `policyHelper` | Team Rules (dashboard, not files) | no | no | no | yes — OS-specific system dirs |
| **Local/personal** | `settings.local.json`, `CLAUDE.local.md` | none formally | `.env` | none formally | `.continuerc.json` | none formally |
| **Composition / imports** | `@path` imports (max depth 5) + tree-walk concat | recursive dir walk | sequential override | alphabetical concat | hub `uses:` imports + dir merge | dir walk + tree to git root |
| **Char/size limits** | none stated | community ~25 lines for always-apply | n/a | none stated | none stated | 6k/file (official), ~12k total (community) |
| **MCP file** | `./.mcp.json` (project), `~/.claude.json` (user), `managed-mcp.json` | `./.cursor/mcp.json`, `~/.cursor/mcp.json` | none (no MCP support) | `cline_mcp_settings.json` (global-only) | `.continue/mcpServers/*.yaml` | `~/.codeium/windsurf/mcp_config.json` |
| **MCP schema** | `{mcpServers: {name: {command, args, env}}}` | same + `url`+`headers` | n/a | same + `disabled`, `autoApprove` | `{name, command, args, type: stdio/sse/streamable-http, env}` | same + `serverUrl`, `headers` |
| **Skills system** | `.claude/skills/*/SKILL.md` | `.cursor/skills/` (added 2.4, Jan 2026) | none | `~/.cline/skills/` | none | `.windsurf/skills/*/SKILL.md` |
| **Subagents** | yes (`.claude/agents/*.md`) | yes (Custom Modes + skills in 2.4) | no | yes (`.cline/agents/`) | no | no |
| **Slash commands** | `.claude/commands/` (unified with skills) | `.cursor/commands/*.md` | built-in only | `/<workflow>.md` files | `.continue/prompts/*.prompt` | `/<workflow>` files |
| **Workflows** | via skills | n/a | n/a | `.clinerules/workflows/*.md` | prompts | `.windsurf/workflows/*.md` |
| **Hooks** | `.claude/hooks/` | `.cursor/hooks/` (announced 2.4) | none | `~/.cline/hooks/` | none | none |
| **Reload behavior** | restart required (root CLAUDE.md re-read on `/compact`) | hot for edits, restart for new files (community-observed) | restart required | rules hot-toggle; MCP often needs VS Code reload | **auto hot-reload on save** | restart for new rules |
| **Precedence: scalars** | managed > session > project-local > project > user | team > project > user | cwd > repo > home (later wins) | workspace > global | global → workspace .continuerc merges per field | system > global > workspace |
| **Precedence: arrays** | concat + dedupe | n/a | n/a | concat | merge (configurable per field) | concat |
| **Deny semantics** | deny always wins across scopes | n/a | n/a | n/a | n/a | n/a |
| **Conflict resolution** | "Claude may pick one arbitrarily" within a file | undefined; "most recently loaded rule" | last file wins | workspace wins | per-field via `mergeBehavior` | workspace wins |
| **Validation tooling** | JSON schema, `/status`, 5 backups auto | none public | none public | none public | published schema (`config_schema.json`) | none public |
| **Notable gotchas** | mid-session edits silently ignored; deny in local can't override managed deny | conflicting rules → undefined | `read:` paths relative to cwd not config | MCP global-only; toggle state doesn't persist | rules bug class — files silently ignored (issue #6905) | hard char limits cause silent drops |

---

## Synthesis: Common Patterns & Divergences for aenv's Adapter Abstraction

### Common patterns to model in the core abstraction

1. **Three-surface model** (per adapter):
   - `instructions`: Markdown-based natural-language rules (CLAUDE.md, .mdc, .clinerules/*, .windsurf/rules/*, CONVENTIONS.md, .continue/rules/*).
   - `settings`: Structured (JSON/YAML) — permissions, model selection, hooks, env.
   - `mcp`: A canonical `{mcpServers: {name: {command, args, env, url, headers, type, disabled, autoApprove}}}` schema, projected to per-tool wrappers.

2. **Five scope layers** (not all tools have all):
   - `managed` (Claude Code, Windsurf system).
   - `global/user` (every tool).
   - `team` (Cursor dashboard, Continue Hub).
   - `project` (every tool).
   - `local/personal` (Claude Code's `*.local.json/md`, .env files).

3. **Frontmatter-based rule activation modes** are now standard. aenv should model `activation: {always | glob | manual | model_decision | path_conditional}` and map to each tool's dialect (Cursor's `alwaysApply: bool` + `globs`; Windsurf's `trigger: ...`; Cline's `paths: [...]`; Continue's `alwaysApply` + `globs` + `regex`).

4. **Cross-tool ingestion is asymmetric but useful**: Continue ingests Cursor/Claude/Cline MCP JSON natively. AGENTS.md is read by Cursor, Windsurf, Cline community, Aider users. aenv profiles should be able to declare a "canonical AGENTS.md" + per-tool overlays.

### Critical divergences aenv must NOT paper over

1. **Reload contract per adapter is non-uniform.** aenv's `aenv use <profile>` command must consult an adapter-level capability:
   ```
   reload: auto-on-save | restart-process | restart-window | mixed-field-dependent | manual-refresh
   ```
   Claude Code, Aider, and Windsurf require process/window restart. Continue auto-reloads. Cursor and Cline are mixed.

2. **MCP scope reality**:
   - **Claude Code, Cursor, Continue, Windsurf**: project-scope MCP supported.
   - **Cline**: global-only — aenv must swap the global file or symlink it from a profile dir.
   - **Aider**: no MCP at all — adapter NOOPs the `mcp` surface.

3. **Merge semantics need per-key handling**, not blanket "override" or "merge":
   - Arrays in Claude Code settings (`permissions.allow`): concat-dedupe across scopes.
   - Deny lists in Claude Code: deny-wins across all scopes (even managed can't be overridden by allow).
   - Scalars in most tools: more-specific scope wins.
   - Continue: `.continuerc.json` per-field `mergeBehavior`.

4. **Silent failure modes are tool-specific** and aenv should validate proactively:
   - Windsurf: 6k/file (official) char budget → silent truncation; ~12k total active (community) → silent drop.
   - Claude Code: invalid keys in settings.json silently ignored; max @-import depth 5.
   - Cline: invalid YAML frontmatter breaks the file silently.
   - Continue: rules bug class where files appear to be loaded but aren't injected (issue #6905).
   - Cursor: conflicting rules produce nondeterministic behavior.

5. **File location absurdities** that hardcoding will break:
   - Cline's VS Code MCP path uses the legacy extension id `saoudrizwan.claude-dev`.
   - Windsurf is `~/.codeium/windsurf/`, not `~/.windsurf/`.
   - Claude Code's `~/.claude.json` (user MCP) is separate from `~/.claude/` (everything else).
   - `CLAUDE_CONFIG_DIR` env var relocates the entire `~/.claude/` tree — aenv adapter must honor it.

### Recommended aenv adapter interface (concrete)

```yaml
adapter: <tool-name>
surfaces:
  instructions:
    format: markdown | mdc | yaml-fm-md
    paths: [{scope: project|user|managed|local, glob: "...", role: "rules|memory|skill|agent|workflow|command|hook"}]
    activation_modes: [always, glob, manual, model_decision]
  settings:
    format: json | yaml | toml
    paths: [...]
    merge_rules:  # per-key
      "permissions.allow": concat-dedupe
      "permissions.deny": concat-dedupe-deny-wins
      "model": override-most-specific
  mcp:
    supported: true|false
    paths: [...]
    schema_dialect: standard | windsurf-headers | cline-extended
behaviors:
  reload: auto-on-save | restart-process | restart-window | mixed
  size_limits: {per_file: bytes, total: bytes}
  validation:
    schema_url: "..."
    silent_failures: [list of known classes]
  scope_precedence: [managed, session, local, project, user, team]
gotchas: [free-text list per tool]
```

---

## Recommendations

1. **Stage 1 — Ship adapters for Claude Code, Cursor, and Aider first.** These three have the cleanest project-scope layouts and the most users. Defer Continue (auto-reload is a nice property but the schema is the most complex) and Cline (MCP global-only is a profile-swap problem aenv users will hit immediately and complain about).

2. **Adopt AGENTS.md as the canonical instruction format inside aenv profiles**, and have adapters fan out to CLAUDE.md, .cursor/rules/agents.mdc, .windsurf/rules/agents.md, etc., via symlink or copy. Reserve tool-specific files for behaviors that don't exist in AGENTS.md (Claude Code permissions, Cursor glob scopes, Windsurf workflows).

3. **Default to symlinks, fall back to copy.** Symlinks preserve a single source of truth and play well with the "reload" hazard (no diff on disk → cleaner snapshot semantics). Cline's VS Code globalStorage path is on the user's home filesystem and supports symlinks; Windows + Cursor sometimes don't follow symlinks across project boundaries — detect and copy when needed.

4. **Build a `aenv doctor` command that runs adapter-specific validators**: Windsurf char-count (warn at 6k/file documented limit), Claude Code @-import depth, JSON schema validation for Continue and Cline MCP, YAML lint for Aider. Validate BEFORE switching, not after.

5. **Surface the reload requirement explicitly in CLI output.** After `aenv use <profile>`, print: "Claude Code: restart session to load new CLAUDE.md / settings.json. Run `aenv reload claude-code` to send SIGHUP if supported." Don't pretend everything is hot-reloadable.

6. **Benchmarks/thresholds that should change the plan**:
   - If Anthropic ships `/reload` for Claude Code (open issues #5513, #15858, #47795) → relax restart requirement for that adapter.
   - If Cline adds project-scope MCP (Discussion #2418) → switch Cline adapter from "swap global file" to "write project file."
   - If `AGENTS.md` becomes universally read (Claude Code is the holdout) → consolidate the `instructions` surface to a single canonical file.
   - If Windsurf removes the 6k character limit → drop the byte-counting validator.

7. **Treat skills and subagents as first-class profile artifacts.** Don't just swap rules — also swap `.claude/agents/`, `.claude/skills/`, `.windsurf/workflows/`, `.cline/agents/`, `.continue/prompts/`. A "frontend-only" profile vs a "backend-API" profile have different subagent rosters, not just different rules.

---

## Caveats

- **Versioning churn is real.** Cursor went from `.cursorrules` → `.cursor/rules/*.mdc` → `.cursor/rules/<name>/RULE.md` (Cursor 2.2+) → added skills + custom modes (2.4, January 2026). Cline launched a CLI-era `~/.cline/` namespace in 2026 in parallel with the legacy `~/Documents/Cline/` and VS Code globalStorage paths. Continue is mid-migration from `config.json` to `config.yaml`. Pin aenv adapter versions to tool versions; ship migration helpers.
- **Reload behavior is partially inferred from community sources.** Continue's hot-reload is documented officially. Claude Code's restart requirement is confirmed by multiple GitHub issues (anthropics/claude-code #15858, #33829, #30737). Cursor's and Windsurf's restart-for-new-rules behavior is community-observed, not documented — aenv should empirically test on each release.
- **Cline's MCP architecture has hot-reload logic in source** (DeepWiki extract confirms `configsRequireRestart()` and a JS file watcher for stdio servers), but community guidance widely says "restart VS Code." Both are partially true: connection-affecting fields trigger automatic reconnects; some installations and config-file location changes don't pick up without a window reload. Treat as "reload-recommended."
- **Windsurf's character limits are partly community-derived.** The 6,000-character per-file limit on `global_rules.md` and `.windsurfrules` is officially documented; the 12,000-character total cap across active rules is community-reported and should not be treated as authoritative — aenv should hard-warn at the documented 6k limit and soft-warn at the community 12k threshold.
- **Enterprise/managed scopes are not testable on free tiers** for Claude Code or Cursor Team Rules. aenv's managed-scope support should be a feature flag, not a default.
- **No tool publishes a stable plugin-API for "config switching."** All of these adapters operate at the file-system level. Tools may add hostile rate limits or signing requirements in future releases — keep the abstraction thin enough to swap.
- **The "skills" landscape is moving fast.** Anthropic published Agent Skills on October 16, 2025 (Anthropic Engineering blog: *"Equipping agents for the real world with Agent Skills"*). Cursor added Agent Skills in version 2.4 (January 2026; *"Define skills in SKILL.md files, which can include custom commands, scripts, and instructions for specializing the agent's capabilities based on the task at hand"*). Windsurf supports SKILL.md across `.windsurf/skills/` and `.agents/skills/`. The SKILL.md file format (folder + SKILL.md + scripts) is becoming a de facto standard — aenv should track agentskills.io as the spec evolves.