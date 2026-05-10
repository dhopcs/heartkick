use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::RwLock;

use crate::api;
use crate::bluetooth;
use crate::config::{self, Config};
use crate::core::{history::SqliteHistory, Engine};
use crate::integrations::Registry;

/// Handle returned to the app so it can interact with the running daemon.
pub struct DaemonHandle {
    pub engine: Arc<Engine>,
    pub config: Arc<RwLock<Config>>,
    pub config_file: std::path::PathBuf,
    pub data_dir: std::path::PathBuf,
}

/// Start the daemon using OS-standard directories (desktop / CLI).
pub async fn start() -> Result<DaemonHandle> {
    let project = config::project_dirs()?;
    let config_dir = project.config_dir().to_path_buf();
    let data_dir = project.data_dir().to_path_buf();
    start_with_paths(config_dir, data_dir).await
}

/// Start the daemon with explicit directory paths (required on mobile where
/// `ProjectDirs` cannot determine OS standard directories before Tauri has
/// initialised the Android environment).
pub async fn start_with_paths(
    config_dir: std::path::PathBuf,
    data_dir: std::path::PathBuf,
) -> Result<DaemonHandle> {
    let config_file = config_dir.join("config.toml");
    std::fs::create_dir_all(&config_dir)
        .with_context(|| format!("creating config dir {}", config_dir.display()))?;
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("creating data dir {}", data_dir.display()))?;

    let config = Config::load_from(&config_file)?;
    let api_cfg = config.api.clone();
    let auto_connect = config.bluetooth.device_address.clone();
    let auto_reconnect = config.bluetooth.auto_reconnect;
    let config = Arc::new(RwLock::new(config));

    let source = bluetooth::default_source();
    let db_path = data_dir.join("history.db");
    let history = Arc::new(SqliteHistory::open(db_path).await?);
    let engine = Engine::new(source, history, config.clone());

    if api_cfg.http_enabled {
        let engine = engine.clone();
        let bind = api_cfg.http_bind.clone();
        let token = api_cfg.api_token.clone();
        tokio::spawn(async move {
            if let Err(e) = api::http::serve(engine, bind, token).await {
                tracing::error!(error = %e, "HTTP API exited");
            }
        });
    }
    if api_cfg.socket_enabled {
        let path = match &api_cfg.socket_path {
            Some(p) => p.clone(),
            None => data_dir.join("heartkick.sock"),
        };
        let engine = engine.clone();
        tokio::spawn(async move {
            if let Err(e) = api::socket::serve(engine, path).await {
                tracing::error!(error = %e, "IPC socket exited");
            }
        });
    }

    // Spawn integrations dispatcher.
    let registry = Registry::from_config(&config.read(), &engine);
    tracing::info!(integrations = ?registry.list(), "integrations loaded");
    registry.spawn(engine.clone());

    // Standalone Prometheus metrics server.
    {
        let prom_cfg = config.read().integrations.prometheus.clone();
        if prom_cfg.enabled {
            let engine_prom = engine.clone();
            let bind = prom_cfg.bind.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::integrations::prometheus::serve(engine_prom, bind).await {
                    tracing::error!(error = %e, "Prometheus server exited");
                }
            });
        }
    }

    // Overlay server.
    {
        let overlay_cfg = config.read().integrations.overlay.clone();
        if overlay_cfg.enabled {
            let engine_overlay = engine.clone();
            let bind = overlay_cfg.bind.clone();
            let custom_html_path = data_dir.join("overlay.html");
            tokio::spawn(async move {
                if let Err(e) =
                    crate::integrations::overlay::serve(engine_overlay, bind, custom_html_path)
                        .await
                {
                    tracing::error!(error = %e, "overlay server exited");
                }
            });
        }
    }

    if let Some(addr) = auto_connect {
        let engine_cl = engine.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = engine_cl.connect(addr.clone()).await {
                    tracing::warn!(error = %e, "auto connect failed");
                    if !auto_reconnect {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
                break;
            }
        });
    }

    Ok(DaemonHandle {
        engine,
        config,
        config_file,
        data_dir,
    })
}

/// Block forever running the daemon.
pub async fn run_forever() -> Result<()> {
    let _handle = start().await?;
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
    Ok(())
}
