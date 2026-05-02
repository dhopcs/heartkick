//! Configuration loaded from a TOML file in the OS standard config directory.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// Top level configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub bluetooth: BluetoothConfig,
    pub api: ApiConfig,
    pub integrations: IntegrationsConfig,
}

/// Which UI to launch on desktop when no `--gui` / `--tui` flag is passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LaunchMode {
    #[default]
    Gui,
    Tui,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub locale: String,
    pub log_level: String,
    /// Desktop-only: which UI to open when the binary is launched without
    /// `--gui` or `--tui`. Ignored on mobile. Defaults to `"gui"`.
    pub launch_mode: LaunchMode,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            log_level: "info".into(),
            launch_mode: LaunchMode::Gui,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BluetoothConfig {
    /// Saved device address. When set, the engine will auto-connect on start.
    pub device_address: Option<String>,
    /// Auto-reconnect when the device drops.
    pub auto_reconnect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    pub http_enabled: bool,
    pub http_bind: String,
    pub socket_enabled: bool,
    /// Optional override for the IPC socket path. When None a default in the OS runtime dir is used.
    pub socket_path: Option<PathBuf>,
    /// When set, every HTTP API request must supply `Authorization: Bearer <token>`.
    pub api_token: Option<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            http_enabled: true,
            http_bind: "127.0.0.1:7878".into(),
            socket_enabled: true,
            socket_path: None,
            api_token: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct IntegrationsConfig {
    pub webhooks: Vec<WebhookConfig>,
    pub prometheus: PrometheusConfig,
    pub osc: OscConfig,
    pub overlay: OverlayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebhookConfig {
    pub name: String,
    pub enabled: bool,
    pub method: String,
    pub url: String,
    /// Header values may use {bpm}, {rr}, {timestamp} substitutions.
    pub headers: std::collections::BTreeMap<String, String>,
    /// Body template. Variables: {bpm}, {rr}, {timestamp}, {device}.
    pub body: String,
    /// Min interval between requests in milliseconds.
    pub min_interval_ms: u64,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            name: "webhook".into(),
            enabled: false,
            method: "POST".into(),
            url: String::new(),
            headers: Default::default(),
            body: r#"{"bpm":{bpm},"timestamp":"{timestamp}"}"#.into(),
            min_interval_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrometheusConfig {
    pub enabled: bool,
    /// Address the standalone Prometheus metrics HTTP server listens on.
    pub bind: String,
    /// Optional push (remote-write) target. When set, metrics are POSTed in
    /// Prometheus text format on every heart rate sample.
    pub push: Option<PrometheusPushConfig>,
}

impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: "127.0.0.1:9090".into(),
            push: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PrometheusPushConfig {
    pub enabled: bool,
    /// URL to POST to, e.g. `http://victoria:8428/api/v1/import/prometheus`.
    pub url: String,
    /// Optional extra HTTP headers (e.g. Authorization).
    pub headers: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OscConfig {
    pub enabled: bool,
    pub target: String,
    pub address: String,
}

impl Default for OscConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            target: "127.0.0.1:9000".into(),
            address: "/heartkick/bpm".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlayConfig {
    pub enabled: bool,
    /// Address the overlay HTTP server listens on.
    pub bind: String,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: "127.0.0.1:9191".into(),
        }
    }
}

/// Returns the project dirs handle.
pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("org", "dhopcs", "heartkick")
        .context("could not determine OS standard directories")
}

/// Path to the config file, ensuring its parent directory exists.
pub fn config_path() -> Result<PathBuf> {
    let dirs = project_dirs()?;
    let dir = dirs.config_dir();
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating config dir {}", dir.display()))?;
    Ok(dir.join("config.toml"))
}

/// Path to the data directory, ensuring it exists.
pub fn data_dir() -> Result<PathBuf> {
    let dirs = project_dirs()?;
    let dir = dirs.data_dir().to_path_buf();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating data dir {}", dir.display()))?;
    Ok(dir)
}

/// Path to the custom overlay HTML file (may not exist; if absent the embedded default is used).
pub fn overlay_html_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("overlay.html"))
}

/// Default IPC socket path inside the runtime / data directory.
pub fn default_socket_path() -> Result<PathBuf> {
    let dirs = project_dirs()?;
    let base = dirs
        .runtime_dir()
        .map(|p| p.to_path_buf())
        .unwrap_or(dirs.data_dir().to_path_buf());
    std::fs::create_dir_all(&base).ok();
    #[cfg(windows)]
    {
        Ok(PathBuf::from(r"\\.\pipe\heartkick"))
    }
    #[cfg(not(windows))]
    {
        Ok(base.join("heartkick.sock"))
    }
}

impl Config {
    /// Load config from an explicit file path.
    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            let cfg = Config::default();
            cfg.save_to(path)?;
            return Ok(cfg);
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
    }

    /// Save config to an explicit file path, creating parent dirs as needed.
    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating config dir {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Load config from the OS-standard location (desktop only).
    pub fn load() -> Result<Self> {
        Self::load_from(&config_path()?)
    }

    /// Save config to the OS-standard location (desktop only).
    pub fn save(&self) -> Result<()> {
        self.save_to(&config_path()?)
    }
}
