//! Terminal UI built with ratatui. Mirrors all pages from the Tauri webview:
//! Home, Devices, Metrics, Logs, Settings.
//!
//! Launch with `heartkick --tui`.

pub mod app;
pub mod ui;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio::time;

use crate::api::controller;
use crate::bluetooth::DeviceInfo;
use crate::config::Config;
use crate::core::Engine;
use app::{App, Tab};

// ── Messages from async tasks back to the event loop ─────────────────────────

enum Msg {
    ScanDone(anyhow::Result<Vec<DeviceInfo>>),
    ConnectDone(anyhow::Result<()>),
}

// ── Public entry point ────────────────────────────────────────────────────────

pub async fn run(
    engine: Arc<Engine>,
    config: Config,
    config_file: PathBuf,
    data_dir: PathBuf,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let result = run_loop(&mut terminal, engine, config, config_file, data_dir).await;

    // Always restore the terminal, even if we errored.
    let _ = disable_raw_mode();
    let _ = terminal.backend_mut().execute(LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result
}

// ── Main event loop ───────────────────────────────────────────────────────────

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    engine: Arc<Engine>,
    config: Config,
    config_file: PathBuf,
    data_dir: PathBuf,
) -> Result<()> {
    let mut app = App::new(engine.clone(), config, config_file, data_dir);
    app.refresh_logs();

    let mut engine_rx = engine.subscribe();
    let mut crossterm_stream = EventStream::new();
    let mut tick = time::interval(Duration::from_millis(500));
    let (msg_tx, mut msg_rx) = mpsc::channel::<Msg>(16);

    terminal.draw(|f| ui::draw(f, &app))?;

    loop {
        tokio::select! {
            biased;

            // Terminal input events
            maybe_event = crossterm_stream.next() => {
                let Some(Ok(event)) = maybe_event else { break };
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        on_key(&mut app, key, &engine, &msg_tx);
                    }
                }
            }

            // Engine broadcast (samples, state changes, session resets)
            res = engine_rx.recv() => {
                use tokio::sync::broadcast::error::RecvError;
                match res {
                    Ok(evt) => app.on_engine_event(evt),
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                }
            }

            // Results from spawned async tasks
            Some(msg) = msg_rx.recv() => {
                match msg {
                    Msg::ScanDone(Ok(devices)) => {
                        app.devices = devices;
                        app.selected_device = 0;
                        app.scanning = false;
                    }
                    Msg::ScanDone(Err(e)) => {
                        app.device_error = Some(e.to_string());
                        app.scanning = false;
                    }
                    Msg::ConnectDone(Ok(())) => {
                        app.connecting = None;
                    }
                    Msg::ConnectDone(Err(e)) => {
                        app.device_error = Some(e.to_string());
                        app.connecting = None;
                    }
                }
            }

            // Periodic tick: refresh logs and snapshot
            _ = tick.tick() => {
                app.refresh_logs();
                app.snapshot = engine.snapshot();
            }
        }

        terminal.draw(|f| ui::draw(f, &app))?;

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

// ── Key dispatch (sync navigation + async action spawn) ───────────────────────

fn on_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    engine: &Arc<Engine>,
    tx: &mpsc::Sender<Msg>,
) {
    // Remember which tab we were on before navigation keys may change it.
    let tab = app.tab;

    // Handle navigation, selection movement, scroll, quit inside App.
    app.on_key(key);

    if app.should_quit {
        return;
    }

    // Dispatch async actions that belong to the tab the key was pressed on.
    use crossterm::event::KeyCode;
    match (tab, key.code) {
        // ── Devices ──────────────────────────────────────────────────────────
        (Tab::Devices, KeyCode::Char('s') | KeyCode::Char('S'))
            if !app.scanning && app.connecting.is_none() =>
        {
            app.scanning = true;
            app.device_error = None;
            app.devices.clear();
            let eng = engine.clone();
            let tx2 = tx.clone();
            tokio::spawn(async move {
                let result = controller::scan(
                    &eng,
                    controller::ScanRequest {
                        timeout_ms: 5000,
                        filter_hr: true,
                    },
                )
                .await;
                let _ = tx2.send(Msg::ScanDone(result)).await;
            });
        }

        // Scan all (including non-HR devices)
        (Tab::Devices, KeyCode::Char('a') | KeyCode::Char('A'))
            if !app.scanning && app.connecting.is_none() =>
        {
            app.scanning = true;
            app.device_error = None;
            app.devices.clear();
            let eng = engine.clone();
            let tx2 = tx.clone();
            tokio::spawn(async move {
                let result = controller::scan(
                    &eng,
                    controller::ScanRequest {
                        timeout_ms: 5000,
                        filter_hr: false,
                    },
                )
                .await;
                let _ = tx2.send(Msg::ScanDone(result)).await;
            });
        }

        (Tab::Devices, KeyCode::Enter)
            if !app.scanning && app.connecting.is_none() && !app.devices.is_empty() =>
        {
            let addr = app.devices[app.selected_device].address.clone();
            app.connecting = Some(addr.clone());
            app.device_error = None;
            let eng = engine.clone();
            let tx2 = tx.clone();
            tokio::spawn(async move {
                let result = eng.connect(addr).await;
                let _ = tx2.send(Msg::ConnectDone(result)).await;
            });
        }

        (Tab::Devices, KeyCode::Char('d') | KeyCode::Char('D')) => {
            let eng = engine.clone();
            tokio::spawn(async move {
                let _ = eng.disconnect().await;
            });
        }

        // ── Metrics ───────────────────────────────────────────────────────────
        (Tab::Metrics, KeyCode::Char('r') | KeyCode::Char('R')) => {
            engine.reset_session();
        }

        _ => {}
    }
}
