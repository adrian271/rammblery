// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
mod ax;

use serde::{Deserialize, Serialize};
use serde_json::json;

#[cfg(target_os = "macos")]
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Suggestion {
    original: String,
    suggestion: String,
    #[serde(rename = "type")]
    kind: String,
    explanation: String,
}

/// Calls the Claude API from the Rust side so the API key never ships
/// inside the frontend bundle. Uses a tool-use call to force structured
/// JSON output instead of parsing free-form prose.
#[tauri::command]
async fn get_suggestions(text: String) -> Result<Vec<Suggestion>, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY is not set — add it to the .env file at the project root".to_string())?;

    let client = reqwest::Client::new();

    let tool_schema = json!({
        "name": "report_suggestions",
        "description": "Report writing suggestions found in the given text.",
        "input_schema": {
            "type": "object",
            "properties": {
                "suggestions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "original": { "type": "string", "description": "Exact substring from the input text that has an issue." },
                            "suggestion": { "type": "string", "description": "The improved replacement text." },
                            "type": { "type": "string", "enum": ["grammar", "clarity", "tone", "concision"] },
                            "explanation": { "type": "string", "description": "One short sentence on why this change helps." }
                        },
                        "required": ["original", "suggestion", "type", "explanation"]
                    }
                }
            },
            "required": ["suggestions"]
        }
    });

    let body = json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1500,
        "tools": [tool_schema],
        "tool_choice": { "type": "tool", "name": "report_suggestions" },
        "messages": [{
            "role": "user",
            "content": format!(
                "Review the following text for grammar errors, unclear phrasing, tone issues, \
                 and unnecessary wordiness. Only flag genuine issues — do not invent changes \
                 for text that is already fine. For each issue, quote the exact original \
                 substring so it can be matched and replaced programmatically.\n\nTEXT:\n{}",
                text
            )
        }]
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Claude API error ({status}): {text}"));
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Bad JSON: {e}"))?;

    let tool_input = data["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find(|b| b["type"] == "tool_use"))
        .and_then(|b| b["input"].as_object())
        .ok_or("No tool_use block in response")?;

    let suggestions: Vec<Suggestion> = serde_json::from_value(
        tool_input.get("suggestions").cloned().unwrap_or(json!([])),
    )
    .map_err(|e| format!("Failed to parse suggestions: {e}"))?;

    Ok(suggestions)
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

    let builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    let builder = {
        // Start disabled; the frontend enables it once permission is granted.
        let ax_control = ax::spawn_ax_thread(false);
        builder.manage(ax_control).invoke_handler(tauri::generate_handler![
            get_suggestions,
            ax_is_trusted,
            ax_request_trust,
            ax_set_enabled
        ])
    };

    #[cfg(not(target_os = "macos"))]
    let builder = builder.invoke_handler(tauri::generate_handler![get_suggestions]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
