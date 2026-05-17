//! Cargonaut configuration schema + loader.
//!
//! Loads from (highest precedence first): CLI args → `CARGONAUT_*` env vars →
//! `~/.config/cargonaut/config.toml` → built-in defaults.
//!
//! Schema is mirror-defined in `contracts/config.schema.json`; this file is
//! the runtime authority.

#![warn(missing_docs)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Top-level configuration tree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Config {
    /// UI settings.
    pub ui: UiConfig,
    /// Transfer engine settings.
    pub transfer: TransferConfig,
    /// Plugin host settings.
    pub plugins: PluginsConfig,
    /// Credential backend selection.
    pub credentials: CredentialsConfig,
    /// Audit log settings.
    pub audit: AuditConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UiConfig::default(),
            transfer: TransferConfig::default(),
            plugins: PluginsConfig::default(),
            credentials: CredentialsConfig::default(),
            audit: AuditConfig::default(),
        }
    }
}

/// UI-related settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct UiConfig {
    /// Theme name.
    pub theme: String,
    /// Enable mouse input.
    pub mouse: bool,
    /// Load MC-compat keymap layer.
    pub mc_keys: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "solarized-dark".into(),
            mouse: false,
            mc_keys: false,
        }
    }
}

/// Transfer engine settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TransferConfig {
    /// Bytes between fsync'd checkpoints.
    pub checkpoint_interval_mib: u32,
    /// Max concurrent transfers.
    pub parallelism: u32,
    /// Re-read destination after copy and verify SHA-256.
    pub verify_after_copy: bool,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            checkpoint_interval_mib: 8,
            parallelism: 4,
            verify_after_copy: true,
        }
    }
}

/// Plugin host settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PluginsConfig {
    /// Plugin names to load.
    pub enabled: Vec<String>,
    /// Allow plugins to use the network.
    pub allow_network: bool,
    /// Allow plugins to spawn subprocesses.
    pub allow_exec: bool,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: vec![],
            allow_network: false,
            allow_exec: false,
        }
    }
}

/// Credential backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CredentialsConfig {
    /// Backend identifier: "system-keychain" | "agent" | "prompt".
    pub backend: String,
}

impl Default for CredentialsConfig {
    fn default() -> Self {
        Self {
            backend: "system-keychain".into(),
        }
    }
}

/// Audit log settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AuditConfig {
    /// Enable audit logging.
    pub enabled: bool,
    /// Rotate daily.
    pub rotate_daily: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rotate_daily: true,
        }
    }
}

impl Config {
    /// Load with full precedence chain. T1.16 implements.
    pub fn load() -> Result<Self, ConfigError> {
        unimplemented!("T1.16 — see design/tasks.md")
    }
}

/// Errors from loading config.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// File not parseable.
    #[error("parse: {0}")]
    Parse(String),
}
