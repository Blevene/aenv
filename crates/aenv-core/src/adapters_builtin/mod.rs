//! Built-in adapters embedded into the binary.
//!
//! Engineering §4: "Built-in adapters ship as embedded TOML strings via
//! `include_str!` and are written to disk on first run. Users can override
//! them by writing a same-named adapter file; the user file wins."

use crate::error::Result;
use crate::fs::Filesystem;
use std::path::Path;

/// The claude-code adapter, embedded at compile time.
pub const CLAUDE_CODE_TOML: &str = include_str!("claude_code.toml");
/// The cursor adapter, embedded at compile time.
pub const CURSOR: &str = include_str!("cursor.toml");
/// The aider adapter, embedded at compile time.
pub const AIDER: &str = include_str!("aider.toml");
/// The cline adapter, embedded at compile time.
pub const CLINE: &str = include_str!("cline.toml");
/// The continue adapter, embedded at compile time.
pub const CONTINUE: &str = include_str!("continue_.toml");
/// The windsurf adapter, embedded at compile time.
pub const WINDSURF: &str = include_str!("windsurf.toml");
/// The mcp adapter, embedded at compile time.
pub const MCP: &str = include_str!("mcp.toml");

/// Every built-in adapter as a (adapter_name, contents) pair.
pub const ALL: &[(&str, &str)] = &[
    ("claude-code", CLAUDE_CODE_TOML),
    ("cursor", CURSOR),
    ("aider", AIDER),
    ("cline", CLINE),
    ("continue", CONTINUE),
    ("windsurf", WINDSURF),
    ("mcp", MCP),
];

/// Every built-in adapter as a (filename, contents) pair.
const BUILTINS: &[(&str, &str)] = &[("claude-code.toml", CLAUDE_CODE_TOML)];

/// Write any built-in adapter that isn't already present on disk into
/// `adapters_dir`. Existing files are left untouched — even if their
/// contents differ from the embedded version — so that a user who has
/// edited their copy keeps their changes.
pub fn install_builtins<F: Filesystem>(fs: &F, adapters_dir: &Path) -> Result<()> {
    fs.create_dir_all(adapters_dir)?;
    for (filename, contents) in BUILTINS {
        let target = adapters_dir.join(filename);
        if fs.exists(&target)? {
            continue;
        }
        fs.write(&target, contents.as_bytes())?;
    }
    Ok(())
}

/// Write every built-in adapter to the registry's adapters dir if not already
/// present. Existing files are left untouched so user edits stick.
pub fn ensure_written<F: Filesystem>(fs: &F, adapters_dir: &Path) -> Result<()> {
    fs.create_dir_all(adapters_dir)?;
    for (name, body) in ALL {
        let path = adapters_dir.join(format!("{name}.toml"));
        if !fs.exists(&path)? {
            fs.write(&path, body.as_bytes())?;
        }
    }
    Ok(())
}
