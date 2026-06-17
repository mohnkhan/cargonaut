// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut configuration schema + loader.
//!
//! Load order (highest precedence first): `CARGONAUT_*` env vars → TOML at
//! the given path (or the default `~/.config/cargonaut/config.toml`) →
//! built-in defaults from this module.
//!
//! The schema is mirror-defined in `design/contracts/config.schema.json`;
//! this module is the runtime authority. JSON Schema can be regenerated
//! via [`Config::json_schema_pretty`].

#![warn(missing_docs)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

// =====================================================================
// Top-level
// =====================================================================

/// Full configuration tree. Every field has a default from
/// `contracts/config.schema.json`; partial TOML fills in only what the
/// user overrides.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// TUI / interaction settings.
    pub ui: UiConfig,
    /// Transfer engine settings.
    pub transfer: TransferConfig,
    /// Plugin host settings.
    pub plugins: PluginsConfig,
    /// Credential backend selection.
    pub credentials: CredentialsConfig,
    /// Audit log settings.
    pub audit: AuditConfig,
    /// Remote-backend settings (SFTP, S3).
    pub remote: RemoteConfig,
    /// Search settings.
    pub search: SearchConfig,
}

// =====================================================================
// UI
// =====================================================================

/// UI-related settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    /// Theme name.
    pub theme: String,
    /// Enable mouse input.
    pub mouse: bool,
    /// Load the orthodox-FM-compat keymap layer.
    pub mc_keys: bool,
    /// Show Unix dotfiles by default.
    pub show_hidden: bool,
    /// `strftime`-style date format for the listing.
    pub date_format: String,
    /// FR-211 zoxide integration tri-state.
    pub zoxide: ZoxideMode,
    /// FR-011 history settings.
    pub history: HistoryConfig,
    /// FR-405 listing-mode settings.
    pub listing: ListingConfig,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            // Feature 031 US1: a built-in palette evoking the reference
            // manager's signature look (was the inert "solarized-dark").
            theme: "commander-dark".into(),
            // Feature 031 US3: mouse on by default so the headline defect
            // is visible on first launch; `--no-mouse` / config disables it.
            mouse: true,
            mc_keys: false,
            show_hidden: false,
            date_format: "%Y-%m-%d %H:%M".into(),
            zoxide: ZoxideMode::Auto,
            history: HistoryConfig::default(),
            listing: ListingConfig::default(),
        }
    }
}

/// FR-211. `auto` enables zoxide iff the binary is on `$PATH` at startup;
/// `true` forces on (error if missing); `false` disables.
///
/// Serializes as the JSON value `"auto"` / `true` / `false` to match the
/// schema's `oneOf: [boolean, "auto"]` shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub enum ZoxideMode {
    /// Enable iff `zoxide` is on `$PATH` at startup.
    Auto,
    /// Force on; error if missing.
    On,
    /// Disable.
    Off,
}

impl Serialize for ZoxideMode {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ZoxideMode::Auto => s.serialize_str("auto"),
            ZoxideMode::On => s.serialize_bool(true),
            ZoxideMode::Off => s.serialize_bool(false),
        }
    }
}

impl<'de> Deserialize<'de> for ZoxideMode {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{self, Visitor};
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = ZoxideMode;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "boolean or the string \"auto\"")
            }
            fn visit_bool<E: de::Error>(self, b: bool) -> Result<ZoxideMode, E> {
                Ok(if b { ZoxideMode::On } else { ZoxideMode::Off })
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<ZoxideMode, E> {
                if s == "auto" {
                    Ok(ZoxideMode::Auto)
                } else {
                    Err(de::Error::custom(format!(
                        "expected boolean or \"auto\", got {s:?}"
                    )))
                }
            }
        }
        d.deserialize_any(V)
    }
}

/// FR-011. Per-pane / global history bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryConfig {
    /// Max in-session directory-history entries per pane (0 = disabled).
    pub directory_depth: u32,
    /// Max persisted command-history entries (0 = disabled).
    pub command_depth: u32,
    /// Where command history is persisted across sessions.
    pub persist_path: String,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            directory_depth: 100,
            command_depth: 1000,
            persist_path: "~/.local/state/cargonaut/history".into(),
        }
    }
}

/// FR-405. Listing-mode settings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ListingConfig {
    /// Initial listing mode; `Alt-t` cycles at runtime.
    pub default_mode: ListingMode,
    /// User-defined column layout (used when `default_mode == User`).
    pub user: UserListingConfig,
}

/// FR-405. Per-pane listing layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ListingMode {
    /// Name-only, multi-column auto-fit.
    Brief,
    /// FR-002 default (name + size + mtime + perms).
    #[default]
    Standard,
    /// One row per file with extended attrs.
    Long,
    /// Columns enumerated in [`UserListingConfig::columns`].
    User,
}

/// FR-405 user-defined column layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct UserListingConfig {
    /// Column ids: built-in (`name|size|mtime|perms|ino|blocks|ctime|atime|xattr|target`)
    /// OR plugin-provided (`plugin-name/column-name`).
    pub columns: Vec<String>,
}

impl Default for UserListingConfig {
    fn default() -> Self {
        Self {
            columns: vec!["name".into(), "size".into(), "perms".into()],
        }
    }
}

// =====================================================================
// Transfer
// =====================================================================

/// Transfer engine settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TransferConfig {
    /// MiB written between fsync'd checkpoints.
    pub checkpoint_interval_mib: u32,
    /// Max concurrent transfers.
    pub parallelism: u32,
    /// Re-read destination after copy and verify SHA-256.
    pub verify_after_copy: bool,
    /// Use `io_uring` on Linux ≥5.10 (Phase 6).
    pub io_uring: bool,
    /// FR-008. Behavior on Ctrl-c cancel of an in-flight transfer.
    pub on_cancel: OnCancel,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            checkpoint_interval_mib: 8,
            parallelism: 4,
            verify_after_copy: true,
            io_uring: true,
            on_cancel: OnCancel::Keep,
        }
    }
}

/// FR-008. On Ctrl-c: delete the partial destination, OR keep it with a
/// checkpoint for resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OnCancel {
    /// Remove the partial destination on cancel.
    Delete,
    /// Keep the partial destination + checkpoint for resume.
    #[default]
    Keep,
}

// =====================================================================
// Plugins
// =====================================================================

/// Plugin host settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PluginsConfig {
    /// Plugin names to load on launch.
    pub enabled: Vec<String>,
    /// Allow plugins to use the network.
    pub allow_network: bool,
    /// Allow plugins to spawn subprocesses.
    pub allow_exec: bool,
    /// Per-plugin wasmtime memory limit.
    pub memory_limit_mib: u32,
    /// Per-host-call wasmtime fuel limit (~100 ms wall).
    pub fuel_limit: u64,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: vec![],
            allow_network: false,
            allow_exec: false,
            memory_limit_mib: 64,
            fuel_limit: 1_000_000_000,
        }
    }
}

// =====================================================================
// Credentials
// =====================================================================

/// Credential backend selection + cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CredentialsConfig {
    /// Backend used for storing remote credentials.
    pub backend: CredentialsBackend,
    /// In-memory password cache duration (`0` = prompt every time).
    pub cache_passwords_for_seconds: u32,
}

impl Default for CredentialsConfig {
    fn default() -> Self {
        Self {
            backend: CredentialsBackend::SystemKeychain,
            cache_passwords_for_seconds: 0,
        }
    }
}

/// Credential backend identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialsBackend {
    /// OS keychain (libsecret / Keychain / wincred).
    #[default]
    SystemKeychain,
    /// SSH agent unix socket.
    Agent,
    /// Interactive prompt only.
    Prompt,
}

// =====================================================================
// Audit
// =====================================================================

/// Audit log settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AuditConfig {
    /// Enable audit logging.
    pub enabled: bool,
    /// Rotate daily.
    pub rotate_daily: bool,
    /// Per-file size cap before rotation.
    pub max_size_mib: u32,
    /// OS-keychain entry name for the audit HMAC key.
    pub hmac_keyring_entry: String,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rotate_daily: true,
            max_size_mib: 64,
            hmac_keyring_entry: "cargonaut/audit-hmac".into(),
        }
    }
}

// =====================================================================
// Remote
// =====================================================================

/// Settings for remote VFS backends.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct RemoteConfig {
    /// SFTP adapter settings.
    pub sftp: SftpConfig,
    /// S3 adapter settings.
    pub s3: S3Config,
}

/// SFTP adapter settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SftpConfig {
    /// TCP connect timeout (seconds).
    pub connect_timeout_secs: u32,
    /// SSH keepalive interval (seconds).
    pub keepalive_secs: u32,
    /// Number of parallel pipelined SFTP read requests (1-16).
    pub pipelined_reads: u32,
}

impl Default for SftpConfig {
    fn default() -> Self {
        Self {
            connect_timeout_secs: 30,
            keepalive_secs: 60,
            pipelined_reads: 4,
        }
    }
}

/// S3 adapter settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct S3Config {
    /// AWS region.
    pub region: String,
    /// Custom S3 endpoint (MinIO, R2, etc.); `None` uses the default for the region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// File size above which multi-part upload is used.
    pub multipart_threshold_mib: u32,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            region: "us-east-1".into(),
            endpoint: None,
            multipart_threshold_mib: 64,
        }
    }
}

// =====================================================================
// Search
// =====================================================================

/// Search settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SearchConfig {
    /// Path to the `rg` (ripgrep) binary; defaults to looking up on `$PATH`.
    pub ripgrep_path: String,
    /// Initial pattern interpretation in the Find dialog.
    pub default_pattern_type: PatternType,
    /// Cap on results before truncation.
    pub max_results: u32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            ripgrep_path: "rg".into(),
            default_pattern_type: PatternType::Glob,
            max_results: 5000,
        }
    }
}

/// Pattern interpretation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PatternType {
    /// Shell-style glob (`*.rs`, `**/foo`).
    #[default]
    Glob,
    /// Rust `regex` crate syntax.
    Regex,
    /// Literal fixed-string search.
    Fixed,
}

// =====================================================================
// Loader
// =====================================================================

/// Env-var prefix for figment overrides. `CARGONAUT_UI__THEME` overrides
/// `ui.theme`; the `__` (double-underscore) is the figment-standard
/// nested-section separator.
const ENV_PREFIX: &str = "CARGONAUT_";

impl Config {
    /// Production loader: defaults → TOML at the default path → `CARGONAUT_*`
    /// env vars. A missing config file is not an error.
    pub fn load() -> Result<Self, ConfigError> {
        let path = default_config_path();
        let mut fig = base_figment();
        if path.exists() {
            use figment::providers::{Format, Toml};
            fig = fig.merge(Toml::file(&path));
        }
        with_env(fig).extract().map_err(figment_err)
    }

    /// Load from the given TOML file path layered on defaults. **Does not**
    /// apply env overrides — that's the job of [`Self::load`] in production.
    /// Errors if the file does not exist or is malformed.
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        use figment::providers::{Format, Toml};
        base_figment()
            .merge(Toml::file(path))
            .extract()
            .map_err(figment_err)
    }

    /// Load from a TOML string layered on defaults. **Does not** apply env
    /// overrides — that's the job of [`Self::load`] in production. Mostly
    /// for tests and the `--config-string` CLI flag.
    pub fn load_from_str(toml_text: &str) -> Result<Self, ConfigError> {
        use figment::providers::{Format, Toml};
        base_figment()
            .merge(Toml::string(toml_text))
            .extract()
            .map_err(figment_err)
    }

    /// Same as [`Self::load_from_str`] but also applies `CARGONAUT_*` env
    /// overrides on top of the TOML. Separated from [`Self::load_from_str`]
    /// because env vars are process-wide global state that breaks parallel
    /// test isolation — opt in explicitly when you want production semantics.
    pub fn load_from_str_with_env(toml_text: &str) -> Result<Self, ConfigError> {
        use figment::providers::{Format, Toml};
        with_env(base_figment().merge(Toml::string(toml_text)))
            .extract()
            .map_err(figment_err)
    }

    /// Render the JSON Schema for [`Config`] as a pretty-printed JSON string.
    /// Mirror of `design/contracts/config.schema.json`; useful for IDE
    /// completion + ad-hoc validation.
    pub fn json_schema_pretty() -> String {
        let schema = schemars::schema_for!(Config);
        serde_json::to_string_pretty(&schema).expect("schemars output is always valid JSON")
    }
}

fn base_figment() -> figment::Figment {
    figment::Figment::from(figment::providers::Serialized::defaults(Config::default()))
}

fn with_env(fig: figment::Figment) -> figment::Figment {
    // Only section-qualified vars (`CARGONAUT_<SECTION>__<FIELD>`, i.e. those
    // containing the `__` nesting separator) are config overrides. The
    // top-level [`Config`] is composed entirely of sub-sections, so a
    // `CARGONAUT_*` var without `__` can never map to a real field. Filtering
    // them out keeps unrelated `CARGONAUT_*` env vars — e.g.
    // `CARGONAUT_PTY_TESTS`, `CARGONAUT_TRANSFER_THROTTLE_MIBPS`,
    // `CARGONAUT_EXIT_CWD_FILE`, `CARGONAUT_ALLOW_SSD_TARGET` — from tripping
    // `deny_unknown_fields` and silently failing the whole config load.
    fig.merge(
        figment::providers::Env::prefixed(ENV_PREFIX)
            .filter(|k| k.as_str().contains("__"))
            .split("__"),
    )
}

fn figment_err(e: figment::Error) -> ConfigError {
    ConfigError::Figment(e.to_string())
}

/// Resolve the default config path. Honors `$XDG_CONFIG_HOME`, falls back
/// to `$HOME/.config/cargonaut/config.toml`; if neither is set, returns
/// a relative path that the caller is unlikely to find — but no panic.
fn default_config_path() -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(xdg).join("cargonaut/config.toml")
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".config/cargonaut/config.toml")
    } else {
        std::path::PathBuf::from(".config/cargonaut/config.toml")
    }
}

// =====================================================================
// Error
// =====================================================================

/// Errors from loading config.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// TOML / JSON parse failure.
    #[error("parse: {0}")]
    Parse(String),

    /// IO error reading the config file.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Figment provider failure (env var conversion, missing file, etc.).
    #[error("figment: {0}")]
    Figment(String),
}

// =====================================================================
// Directory hotlist / bookmarks (Feature 042, issue #42)
// =====================================================================

/// A named shortcut to a directory. `path` is a path/URI string in the same
/// form [`crate`]-consuming navigation accepts; `group` is an optional
/// single-level category (`None` ⇒ shown in the default/ungrouped section).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    /// User-visible label (non-empty).
    pub name: String,
    /// Target directory (path or `file://` URI).
    pub path: String,
    /// Optional group/category label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// One group's bucket from [`Hotlist::grouped`]: the group key (`None` =
/// ungrouped) paired with its bookmarks, each carrying its original index.
pub type HotlistGroup<'a> = (Option<&'a str>, Vec<(usize, &'a Bookmark)>);

/// The user's directory hotlist: an ordered collection of [`Bookmark`]s,
/// persisted as a TOML state file (see [`default_hotlist_path`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotlist {
    /// Bookmarks in insertion order. Serialized as `[[bookmark]]` tables.
    #[serde(default, rename = "bookmark")]
    pub bookmarks: Vec<Bookmark>,
}

impl Hotlist {
    /// Load the hotlist from `path`. **Never errors**: a missing, unreadable,
    /// or malformed file yields an empty hotlist (FR-007/FR-013) — a corrupt
    /// state file must never block launch.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist the hotlist to `path` as TOML, creating parent directories as
    /// needed. Whole-file rewrite (last-write-wins).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, text)
    }

    /// Append a bookmark.
    pub fn add(&mut self, bookmark: Bookmark) {
        self.bookmarks.push(bookmark);
    }

    /// Remove the bookmark at `index`. Out-of-range is a silent no-op.
    pub fn remove(&mut self, index: usize) {
        if index < self.bookmarks.len() {
            self.bookmarks.remove(index);
        }
    }

    /// Display projection: bookmarks bucketed by group, each entry carrying its
    /// original index (so the popup can map a selection back to a bookmark).
    /// Groups appear in first-seen order; the ungrouped (`None`) section, if
    /// any, is placed last. (FR-014 / SC-007.)
    pub fn grouped(&self) -> Vec<HotlistGroup<'_>> {
        let mut order: Vec<Option<&str>> = Vec::new();
        let mut buckets: Vec<HotlistGroup<'_>> = Vec::new();
        for (i, b) in self.bookmarks.iter().enumerate() {
            let key = b.group.as_deref();
            let pos = match order.iter().position(|k| *k == key) {
                Some(p) => p,
                None => {
                    order.push(key);
                    buckets.push((key, Vec::new()));
                    buckets.len() - 1
                }
            };
            buckets[pos].1.push((i, b));
        }
        // Ungrouped section last.
        buckets.sort_by_key(|(k, _)| k.is_none());
        buckets
    }
}

/// Resolve the default hotlist state-file path. Honors `$XDG_STATE_HOME`, falls
/// back to `$HOME/.local/state/cargonaut/hotlist.toml`; if neither is set,
/// returns a relative path (no panic). Mirrors [`default_config_path`] but uses
/// the XDG **state** dir (the hotlist is machine-written state, not config).
pub fn default_hotlist_path() -> std::path::PathBuf {
    hotlist_path_from(
        std::env::var("XDG_STATE_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
}

/// Pure resolver behind [`default_hotlist_path`] (env values injected for tests).
fn hotlist_path_from(xdg_state: Option<&str>, home: Option<&str>) -> std::path::PathBuf {
    if let Some(xdg) = xdg_state {
        std::path::PathBuf::from(xdg).join("cargonaut/hotlist.toml")
    } else if let Some(home) = home {
        std::path::PathBuf::from(home).join(".local/state/cargonaut/hotlist.toml")
    } else {
        std::path::PathBuf::from(".local/state/cargonaut/hotlist.toml")
    }
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ===== Feature 042: directory hotlist / bookmarks =====

    #[test]
    fn hotlist_round_trip_preserves_entries_incl_group() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hotlist.toml");
        let mut hl = Hotlist::default();
        hl.add(Bookmark {
            name: "proj".into(),
            path: "file:///home/u/work/proj".into(),
            group: Some("work".into()),
        });
        hl.add(Bookmark {
            name: "tmp".into(),
            path: "file:///tmp".into(),
            group: None,
        });
        hl.save(&path).unwrap();
        let loaded = Hotlist::load(&path);
        assert_eq!(loaded, hl);
    }

    #[test]
    fn hotlist_load_absent_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.toml");
        assert_eq!(Hotlist::load(&path), Hotlist::default());
    }

    #[test]
    fn hotlist_load_malformed_is_empty_no_panic() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"this is not valid toml :::: [[[").unwrap();
        assert_eq!(Hotlist::load(f.path()), Hotlist::default());
    }

    #[test]
    fn hotlist_save_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deeper/hotlist.toml");
        Hotlist::default().save(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn hotlist_path_resolution() {
        assert_eq!(
            hotlist_path_from(Some("/x/state"), Some("/h")),
            std::path::PathBuf::from("/x/state/cargonaut/hotlist.toml")
        );
        assert_eq!(
            hotlist_path_from(None, Some("/h")),
            std::path::PathBuf::from("/h/.local/state/cargonaut/hotlist.toml")
        );
        assert_eq!(
            hotlist_path_from(None, None),
            std::path::PathBuf::from(".local/state/cargonaut/hotlist.toml")
        );
    }

    #[test]
    fn hotlist_grouped_buckets_with_ungrouped_default_and_indices() {
        let mut hl = Hotlist::default();
        hl.add(Bookmark {
            name: "a".into(),
            path: "/a".into(),
            group: Some("work".into()),
        }); // idx 0
        hl.add(Bookmark {
            name: "b".into(),
            path: "/b".into(),
            group: None,
        }); // idx 1
        hl.add(Bookmark {
            name: "c".into(),
            path: "/c".into(),
            group: Some("work".into()),
        }); // idx 2
        let grouped = hl.grouped();
        // work group carries a + c with their original indices.
        let work = grouped
            .iter()
            .find(|(g, _)| *g == Some("work"))
            .expect("work group present");
        assert_eq!(
            work.1.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            vec![0, 2]
        );
        // ungrouped (None) carries b at original index 1.
        let ungrouped = grouped
            .iter()
            .find(|(g, _)| g.is_none())
            .expect("ungrouped section present");
        assert_eq!(
            ungrouped.1.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            vec![1]
        );
    }

    #[test]
    fn defaults_have_documented_values() {
        let c = Config::default();
        // UI
        assert_eq!(c.ui.theme, "commander-dark");
        assert!(c.ui.mouse);
        assert!(!c.ui.mc_keys);
        assert!(!c.ui.show_hidden);
        assert_eq!(c.ui.date_format, "%Y-%m-%d %H:%M");
        assert_eq!(c.ui.zoxide, ZoxideMode::Auto);
        assert_eq!(c.ui.history.directory_depth, 100);
        assert_eq!(c.ui.history.command_depth, 1000);
        assert_eq!(
            c.ui.history.persist_path,
            "~/.local/state/cargonaut/history"
        );
        assert_eq!(c.ui.listing.default_mode, ListingMode::Standard);
        assert_eq!(c.ui.listing.user.columns, vec!["name", "size", "perms"]);
        // Transfer
        assert_eq!(c.transfer.checkpoint_interval_mib, 8);
        assert_eq!(c.transfer.parallelism, 4);
        assert!(c.transfer.verify_after_copy);
        assert!(c.transfer.io_uring);
        assert_eq!(c.transfer.on_cancel, OnCancel::Keep);
        // Plugins
        assert!(c.plugins.enabled.is_empty());
        assert!(!c.plugins.allow_network);
        assert!(!c.plugins.allow_exec);
        assert_eq!(c.plugins.memory_limit_mib, 64);
        assert_eq!(c.plugins.fuel_limit, 1_000_000_000);
        // Credentials
        assert_eq!(c.credentials.backend, CredentialsBackend::SystemKeychain);
        assert_eq!(c.credentials.cache_passwords_for_seconds, 0);
        // Audit
        assert!(c.audit.enabled);
        assert!(c.audit.rotate_daily);
        assert_eq!(c.audit.max_size_mib, 64);
        assert_eq!(c.audit.hmac_keyring_entry, "cargonaut/audit-hmac");
        // Remote
        assert_eq!(c.remote.sftp.connect_timeout_secs, 30);
        assert_eq!(c.remote.sftp.keepalive_secs, 60);
        assert_eq!(c.remote.sftp.pipelined_reads, 4);
        assert_eq!(c.remote.s3.region, "us-east-1");
        assert!(c.remote.s3.endpoint.is_none());
        assert_eq!(c.remote.s3.multipart_threshold_mib, 64);
        // Search
        assert_eq!(c.search.ripgrep_path, "rg");
        assert_eq!(c.search.default_pattern_type, PatternType::Glob);
        assert_eq!(c.search.max_results, 5000);
    }

    #[test]
    fn defaults_round_trip_through_toml() {
        let c = Config::default();
        let s = toml::to_string(&c).unwrap();
        let c2: Config = toml::from_str(&s).unwrap();
        assert_eq!(c, c2);
    }

    #[test]
    fn defaults_round_trip_through_json() {
        let c = Config::default();
        let s = serde_json::to_string(&c).unwrap();
        let c2: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(c, c2);
    }

    #[test]
    fn load_from_str_with_partial_toml_fills_defaults() {
        let toml_text = r#"
[ui]
theme = "dracula"
mc_keys = true

[transfer]
parallelism = 8
"#;
        let c = Config::load_from_str(toml_text).unwrap();
        assert_eq!(c.ui.theme, "dracula");
        assert!(c.ui.mc_keys);
        // Feature 031: mouse now defaults ON; partial TOML leaves it unset → true.
        assert!(c.ui.mouse);
        assert_eq!(c.transfer.parallelism, 8);
        assert_eq!(c.transfer.checkpoint_interval_mib, 8);
    }

    #[test]
    fn load_from_path_reads_toml_file() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(
            br#"
[plugins]
enabled = ["git-status"]
"#,
        )
        .unwrap();
        let c = Config::load_from_path(f.path()).unwrap();
        assert_eq!(c.plugins.enabled, vec!["git-status".to_string()]);
    }

    #[test]
    #[allow(clippy::result_large_err)] // figment::Jail's API returns figment::Error directly
    fn env_var_overrides_toml() {
        // Uses load_from_str_with_env, not load_from_str — env vars are
        // process-wide global state and only the _with_env variant reads
        // them, so this test can't pollute parallel siblings.
        figment::Jail::expect_with(|jail| {
            jail.set_env("CARGONAUT_UI__THEME", "monochrome");
            let toml_text = r#"
[ui]
theme = "dracula"
"#;
            let c = Config::load_from_str_with_env(toml_text).unwrap();
            assert_eq!(c.ui.theme, "monochrome");
            Ok(())
        });
    }

    #[test]
    // The figment::Jail closure returns Result<(), figment::Error>; the Err
    // size is irrelevant in a test harness.
    #[allow(clippy::result_large_err)]
    fn unrelated_cargonaut_env_var_does_not_break_load() {
        // Regression (Feature 037): non-config `CARGONAUT_*` vars (no `__`
        // section separator) must be ignored, not fed into the
        // deny_unknown_fields extract. Setting CARGONAUT_PTY_TESTS used to
        // make the whole config load fail.
        figment::Jail::expect_with(|jail| {
            jail.set_env("CARGONAUT_PTY_TESTS", "1");
            jail.set_env("CARGONAUT_TRANSFER_THROTTLE_MIBPS", "24");
            jail.set_env("CARGONAUT_UI__THEME", "monochrome");
            let c = Config::load_from_str_with_env("").unwrap();
            // The valid override still applies; the stray vars are ignored.
            assert_eq!(c.ui.theme, "monochrome");
            Ok(())
        });
    }

    #[test]
    fn unknown_field_is_rejected() {
        let toml_text = r#"
[ui]
theme = "dracula"
wibble = 42
"#;
        let res = Config::load_from_str(toml_text);
        assert!(res.is_err(), "deny_unknown_fields should reject 'wibble'");
    }

    #[test]
    fn json_schema_includes_all_top_level_sections() {
        let schema = Config::json_schema_pretty();
        for section in &[
            "ui",
            "transfer",
            "plugins",
            "credentials",
            "audit",
            "remote",
            "search",
        ] {
            assert!(
                schema.contains(&format!("\"{section}\"")),
                "missing section: {section}"
            );
        }
    }

    #[test]
    fn zoxide_mode_serde_round_trip() {
        for mode in [ZoxideMode::Auto, ZoxideMode::On, ZoxideMode::Off] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: ZoxideMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn zoxide_mode_accepts_auto_string_and_bool() {
        assert_eq!(
            serde_json::from_str::<ZoxideMode>(r#""auto""#).unwrap(),
            ZoxideMode::Auto
        );
        assert_eq!(
            serde_json::from_str::<ZoxideMode>("true").unwrap(),
            ZoxideMode::On
        );
        assert_eq!(
            serde_json::from_str::<ZoxideMode>("false").unwrap(),
            ZoxideMode::Off
        );
        assert!(serde_json::from_str::<ZoxideMode>(r#""bogus""#).is_err());
    }

    // ===== Feature 047: user menu config types =====

    #[test]
    fn menu_item_full_deserialization() {
        let toml_text = r#"
[[actions]]
label   = "Edit"
command = "$EDITOR {path}"
only_if = "test -f {path}"
key     = "e"
"#;
        let cfg: UserMenuConfig = toml::from_str(toml_text).unwrap();
        assert_eq!(cfg.actions.len(), 1);
        let item = &cfg.actions[0];
        assert_eq!(item.label, "Edit");
        assert_eq!(item.command, "$EDITOR {path}");
        assert_eq!(item.only_if, Some("test -f {path}".into()));
        assert_eq!(item.key, Some('e'));
    }

    #[test]
    fn menu_item_only_required_fields() {
        let toml_text = r#"
[[actions]]
label   = "Do something"
command = "echo hello"
"#;
        let cfg: UserMenuConfig = toml::from_str(toml_text).unwrap();
        assert_eq!(cfg.actions.len(), 1);
        assert!(cfg.actions[0].only_if.is_none());
        assert!(cfg.actions[0].key.is_none());
    }

    #[test]
    fn menu_config_empty_actions_array() {
        let toml_text = "actions = []\n";
        let cfg: UserMenuConfig = toml::from_str(toml_text).unwrap();
        assert!(cfg.actions.is_empty());
    }

    #[test]
    fn menu_config_empty_toml_gives_empty_actions() {
        let cfg: UserMenuConfig = toml::from_str("").unwrap();
        assert!(cfg.actions.is_empty());
    }
}
