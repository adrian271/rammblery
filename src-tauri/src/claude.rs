//! Claude Messages API client for writing suggestions.
//!
//! Uses a forced tool-use call so Claude returns structured JSON (parsed into
//! `Suggestion`) instead of prose. Shared by the paste-in editor command and
//! the system-wide checker.

use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Suggestion {
    pub original: String,
    pub suggestion: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub explanation: String,
}

const MODEL: &str = "claude-sonnet-4-6";

/// Run one suggestion pass over `text`. The caller owns the `reqwest::Client`
/// so it can be reused (and so an in-flight request cancels when dropped).
pub async fn fetch_suggestions(
    client: &reqwest::Client,
    text: &str,
) -> Result<Vec<Suggestion>, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        "ANTHROPIC_API_KEY is not set — add it to the .env file at the project root".to_string()
    })?;

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
        "model": MODEL,
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

    let suggestions: Vec<Suggestion> =
        serde_json::from_value(tool_input.get("suggestions").cloned().unwrap_or(json!([])))
            .map_err(|e| format!("Failed to parse suggestions: {e}"))?;

    Ok(suggestions)
}
