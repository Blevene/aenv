# Security Policy

## Supported versions

While `aenv` is pre-1.0 (`0.0.x`), security fixes ship in the next patch release. The current release line is the only supported one — older `0.0.x` tags don't receive backports.

| Version | Supported |
|---|---|
| `0.0.2` (latest) | ✅ |
| `0.0.1` | ❌ — upgrade to `0.0.2` |

## Reporting a vulnerability

**Please don't open a public GitHub issue for security issues.** Send a private report to **blevene@lightforgeventures.com** with the subject line `aenv security`.

Include:

- A description of the vulnerability and its impact
- Steps to reproduce (commands, env vars, namespace fixtures if any)
- The version of `aenv` (`aenv --version`) and your OS
- Any proof-of-concept code or attack scenario you've developed

You'll get an acknowledgment within **7 days**. From there:

- If the issue is confirmed, the maintainer will work on a fix and coordinate a disclosure timeline with you.
- If the issue is out of scope or a non-issue, you'll get an explanation within the same timeframe.

GitHub's [private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability) is also enabled on this repo as an alternative submission channel.

## Threat model — what aenv does and doesn't claim

`aenv` materializes namespace content into project directories — primarily as symlinks. The threat surface that matters:

- **`AENV_HOME` (`~/.aenv/` by default) is trusted.** Anyone who can write to `AENV_HOME` can place arbitrary content in a project on the next `aenv activate`. Don't share `AENV_HOME` across trust boundaries (e.g., don't point multiple users at the same `AENV_HOME`).
- **Imported skills run user-trusted code.** `aenv skill import git+<url>` shallow-clones the source on `aenv activate`. The clone is not sandboxed; whatever the source repo's `SKILL.md` says, the agent will read. Pin with `--pin <ref>` and audit upstream before importing.
- **`.aenv-state/backup/` holds the user's pre-activation files** during a namespace's lifetime. Treat it the same as the project itself — don't share it.
- **`aenv` does not execute arbitrary code from manifests.** `aenv.toml` is data-only TOML; there's no shell-out or eval path in the parser.

Issues *in scope*:

- Path traversal in any file-resolution path (`.aenv` pin walk, `--path` validation, skill cache lookup, snapshot)
- TOCTOU windows in activate/deactivate (file replaced between check and use)
- Symlink-attack escalations (e.g., a malicious namespace luring `aenv activate` into following a symlink out of the project)
- Cache poisoning between namespaces sharing the same `~/.aenv/cache/skills/<hash>/<ref>/`
- Integrity bypasses (resolved-namespace hash not actually catching what it claims to)

Issues *out of scope* for now:

- Anything depending on a malicious local user already having write access to `AENV_HOME` or the project root — that's pre-compromise.
- Network MITM against `git clone` — `aenv` defers to the system `git`, which inherits whatever transport security `git` provides.
- DoS via large namespace trees / huge `extends` chains — bounded by user input.

## Acknowledgment

If you report an issue and it leads to a fix, you'll be credited in `CHANGELOG.md` (unless you'd rather not).
