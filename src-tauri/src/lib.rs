pub mod api;
pub mod bluetooth;
pub mod cli;
pub mod config;
pub mod core;
pub mod daemon;
pub mod integrations;
pub mod logs;
#[cfg(feature = "gui")]
pub mod tauri_commands;

#[cfg(feature = "gui")]
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

/// Run heartkick in app mode: starts the embedded daemon and the Tauri webview.
#[cfg(feature = "gui")]
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

    let rh = runtime.handle().clone();

    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(any(target_os = "ios", target_os = "android"))]
    let builder = builder.plugin(tauri_plugin_blec::init());

    builder
        .setup(move |app| {
            use tauri::Manager;
            let config_dir = app.path().app_config_dir().expect("app_config_dir");
            let data_dir = app.path().app_data_dir().expect("app_data_dir");
            let handle = rh
                .block_on(daemon::start_with_paths(config_dir, data_dir))
                .expect("daemon start");
            let app_state = cmds::AppState {
                engine: handle.engine.clone(),
                config: handle.config.clone(),
                config_file: handle.config_file.clone(),
                data_dir: handle.data_dir.clone(),
            };
            app.manage(app_state);
            cmds::forward_events(&app.handle(), handle.engine.clone());

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
