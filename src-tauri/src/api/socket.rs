use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::api::controller;
use crate::core::Engine;

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Command {
    Snapshot,
    Scan { timeout_ms: Option<u64> },
    Connect { address: String },
    Disconnect,
    ResetSession,
    History { limit: Option<usize> },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Reply {
    Snapshot(crate::core::EngineSnapshot),
    Scan(Vec<crate::bluetooth::DeviceInfo>),
    History(controller::HistoryResponse),
    Ok,
    Error { message: String },
}

#[cfg(unix)]
pub async fn serve(engine: Arc<Engine>, path: std::path::PathBuf) -> Result<()> {
    use tokio::net::UnixListener;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("binding unix socket {}", path.display()))?;
    tracing::info!(path = %path.display(), "IPC socket listening");
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "accept failed");
                continue;
            }
        };
        let engine = engine.clone();
        tokio::spawn(handle(stream, engine));
    }
}

#[cfg(windows)]
pub async fn serve(engine: Arc<Engine>, path: std::path::PathBuf) -> Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;
    let pipe_name = path.to_string_lossy().to_string();
    tracing::info!(pipe = %pipe_name, "IPC pipe listening");
    loop {
        let server = ServerOptions::new()
            .first_pipe_instance(false)
            .create(&pipe_name)
            .context("creating named pipe instance")?;
        server.connect().await.context("waiting for pipe client")?;
        let engine = engine.clone();
        tokio::spawn(handle(server, engine));
    }
}

async fn handle<S>(stream: S, engine: Arc<Engine>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half).lines();

    // Pump engine events to the client.
    let mut rx = engine.subscribe();
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<String>(64);
    let writer_tx = out_tx.clone();
    tokio::spawn(async move {
        while let Ok(evt) = rx.recv().await {
            if let Ok(line) = serde_json::to_string(&evt) {
                if writer_tx.send(format!("{line}\n")).await.is_err() {
                    break;
                }
            }
        }
    });

    let writer = tokio::spawn(async move {
        while let Some(line) = out_rx.recv().await {
            if write_half.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
    });

    while let Ok(Some(line)) = reader.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Command>(trimmed) {
            Ok(cmd) => dispatch(&engine, cmd).await,
            Err(e) => Reply::Error {
                message: format!("invalid command: {e}"),
            },
        };
        if let Ok(line) = serde_json::to_string(&reply) {
            if out_tx.send(format!("{line}\n")).await.is_err() {
                break;
            }
        }
    }
    drop(out_tx);
    let _ = writer.await;
}

async fn dispatch(engine: &Arc<Engine>, cmd: Command) -> Reply {
    match cmd {
        Command::Snapshot => Reply::Snapshot(controller::snapshot(engine)),
        Command::Scan { timeout_ms } => {
            match controller::scan(
                engine,
                controller::ScanRequest {
                    timeout_ms: timeout_ms.unwrap_or(5000),
                    filter_hr: true,
                },
            )
            .await
            {
                Ok(d) => Reply::Scan(d),
                Err(e) => Reply::Error {
                    message: e.to_string(),
                },
            }
        }
        Command::Connect { address } => {
            match controller::connect(engine, controller::ConnectRequest { address }).await {
                Ok(()) => Reply::Ok,
                Err(e) => Reply::Error {
                    message: e.to_string(),
                },
            }
        }
        Command::Disconnect => {
            let _ = controller::disconnect(engine).await;
            Reply::Ok
        }
        Command::ResetSession => {
            controller::reset_session(engine);
            Reply::Ok
        }
        Command::History { limit } => {
            Reply::History(controller::history(engine, limit.unwrap_or(300)).await)
        }
    }
}
