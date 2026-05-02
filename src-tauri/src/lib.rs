//! heartkick library entry. Exposes the modules used by both the binary and
//! the Tauri runtime, and provides [`run`] for the app mode.

pub mod api;
pub mod bluetooth;
pub mod cli;
pub mod config;
pub mod core;
pub mod daemon;
pub mod integrations;
pub mod logs;
pub mod tauri_commands;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub mod tui;

use tauri_commands as cmds;

/// Initialize tracing once. Safe to call multiple times.
pub fn init_tracing(level: Option<&str>) {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
    let filter = level.map(EnvFilter::new).unwrap_or_else(|| {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    });
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(logs::LogLayer)
        .try_init();
}

/// Initialize tracing for TUI mode: only feeds the in-process ring buffer so
/// that log output never prints over the terminal UI.
pub fn init_tracing_tui(level: Option<&str>) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
    let filter = level.map(EnvFilter::new).unwrap_or_else(|| {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    });
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(logs::LogLayer)
        .try_init();
}

/// Run heartkick in app mode: starts the embedded daemon and the Tauri webview.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing(None);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(512 * 1024)
        .enable_all()
        .build()
        .expect("tokio runtime");

    // Hand the runtime to Tauri before building the app so async commands work.
    tauri::async_runtime::set(runtime.handle().clone());

    // On desktop we can resolve OS-standard directories without a Tauri context,
    // so we start the daemon now and pass the engine into the builder directly.
    // On mobile (Android/iOS) ProjectDirs cannot determine directories until
    // Tauri has initialised the platform environment, so we defer daemon startup
    // to the setup hook where the path resolver is available.
    #[cfg(not(mobile))]
    let pre_handle = runtime.block_on(async { daemon::start().await.expect("daemon start") });

    let _rh = runtime.handle().clone();
    #[cfg(mobile)]
    let rh = _rh;

    #[cfg(not(mobile))]
    let state = cmds::AppState {
        engine: pre_handle.engine.clone(),
        config: pre_handle.config.clone(),
        config_file: pre_handle.config_file.clone(),
        data_dir: pre_handle.data_dir.clone(),
    };

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_blec::init())
        .plugin(tauri_plugin_opener::init());

    #[cfg(not(mobile))]
    {
        builder = builder.manage(state);
    }

    builder
        .setup(move |app| {
            #[cfg(not(mobile))]
            {
                cmds::forward_events(app.handle(), pre_handle.engine.clone());
            }

            #[cfg(mobile)]
            {
                use tauri::Manager;
                let config_dir = app.path().app_config_dir().expect("app_config_dir");
                let data_dir = app.path().app_data_dir().expect("app_data_dir");
                let handle = rh
                    .block_on(daemon::start_with_paths(config_dir, data_dir))
                    .expect("daemon start");
                let mobile_state = cmds::AppState {
                    engine: handle.engine.clone(),
                    config: handle.config.clone(),
                    config_file: handle.config_file.clone(),
                    data_dir: handle.data_dir.clone(),
                };
                app.manage(mobile_state);
                cmds::forward_events(&app.handle(), handle.engine.clone());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmds::snapshot,
            cmds::scan,
            cmds::connect,
            cmds::disconnect,
            cmds::reset_session,
            cmds::history,
            cmds::get_config,
            cmds::save_config,
            cmds::config_paths,
            cmds::save_device,
            cmds::get_logs,
            cmds::get_overlay_html,
            cmds::save_overlay_html,
            cmds::reset_overlay_html,
            cmds::get_default_overlay_html,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // Keep the runtime alive for the duration of run().
    drop(runtime);
}
