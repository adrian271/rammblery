// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod claude;

#[cfg(target_os = "macos")]
mod ax;
#[cfg(target_os = "macos")]
mod checker;
#[cfg(target_os = "macos")]
mod panel;

use claude::Suggestion;

#[cfg(target_os = "macos")]
use std::sync::Arc;

/// Calls the Claude API from the Rust side so the API key never ships
/// inside the frontend bundle (backs the paste-in editor).
#[tauri::command]
async fn get_suggestions(text: String) -> Result<Vec<Suggestion>, String> {
    let client = reqwest::Client::new();
    claude::fetch_suggestions(&client, &text).await
}

/// macOS Accessibility: is the app currently trusted (permission granted)?
#[cfg(target_os = "macos")]
#[tauri::command]
fn ax_is_trusted() -> bool {
    ax::permission::is_trusted()
}

/// macOS Accessibility: prompt for permission (and open Settings as a fallback
/// for when the one-shot system prompt was already dismissed).
#[cfg(target_os = "macos")]
#[tauri::command]
fn ax_request_trust() -> bool {
    let trusted = ax::permission::request_trust();
    if !trusted {
        ax::permission::open_settings();
    }
    trusted
}

/// macOS Accessibility: enable/disable the system-wide checking poll loop.
#[cfg(target_os = "macos")]
#[tauri::command]
fn ax_set_enabled(state: tauri::State<'_, Arc<ax::AxControl>>, enabled: bool) {
    state.set_enabled(enabled);
}

fn main() {
    // Loads .env from the project root (dotenvy walks up parent dirs);
    // fine if missing — the key may come from the shell environment instead.
    let _ = dotenvy::dotenv();

    // Simple logger so the AX thread's info! output reaches the terminal.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let builder = tauri::Builder::default().setup(|_app| {
        #[cfg(target_os = "macos")]
        {
            use tauri::Manager;

            // AX thread → checker event channel.
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

            // The floating suggestion panel (hidden until a check produces results).
            panel::create(_app.handle())?;

            // Start the AX thread disabled; the frontend enables it once
            // Accessibility permission is granted.
            let ax_control = ax::spawn_ax_thread(false, tx);
            _app.manage(ax_control);

            // The checker consumes AX events, debounces, calls Claude, and
            // drives the panel.
            let handle = _app.handle().clone();
            tauri::async_runtime::spawn(checker::run(handle, rx));
        }
        Ok(())
    });

    #[cfg(target_os = "macos")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        get_suggestions,
        ax_is_trusted,
        ax_request_trust,
        ax_set_enabled
    ]);

    #[cfg(not(target_os = "macos"))]
    let builder = builder.invoke_handler(tauri::generate_handler![get_suggestions]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
