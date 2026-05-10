// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use heartkick_lib::cli::Cli;

fn run_daemon_forever(log_level: Option<&str>) {
    heartkick_lib::init_tracing(log_level);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(512 * 1024)
        .enable_all()
        .build()
        .expect("tokio runtime");
    if let Err(e) = runtime.block_on(heartkick_lib::daemon::run_forever()) {
        eprintln!("daemon error: {e}");
        std::process::exit(1);
    }
}

fn main() {
    // WebKitGTK on Wayland compositor can fail with "Error 71 (Protocol error)".
    // These vars must be set before GTK/GDK initialises
    // Users can override either variable in their environment before launching.
    #[cfg(all(target_os = "linux", feature = "gui"))]
    {
        if std::env::var("GDK_BACKEND").is_err() {
            // Fall back to XWayland; avoids Wayland compositor protocol errors.
            // SAFETY: single-threaded at this point, no other thread can read env.
            unsafe { std::env::set_var("GDK_BACKEND", "x11") };
        }
        if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
            unsafe { std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1") };
        }
    }

    let cli = Cli::parse();

    if cli.daemon {
        run_daemon_forever(cli.log.as_deref());
        return;
    }

    #[cfg(feature = "gui")]
    {
        heartkick_lib::run();
    }

    #[cfg(not(feature = "gui"))]
    {
        run_daemon_forever(cli.log.as_deref());
    }
}
