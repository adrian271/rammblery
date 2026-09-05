//! Bridges AX events to the Claude pipeline: debounce, cancel stale work, and
//! push results to the floating panel.
//!
//! Runs as one tokio task. Debounce lives here (not the frontend) since the
//! source is now the AX thread. A newer text change aborts any in-flight
//! request and restarts the debounce; identical text is not re-checked.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use tauri::AppHandle;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio::time::{Instant, Sleep};

use crate::ax::{AxEvent, Frame};
use crate::{claude, panel};

const DEBOUNCE: Duration = Duration::from_millis(1500);
/// Far-future deadline used to "disarm" the debounce timer.
const IDLE: Duration = Duration::from_secs(3600);

struct Pending {
    session_id: u64,
    text: String,
    frame: Option<Frame>,
}

/// Result of one completed (non-aborted) check.
struct CheckDone {
    session_id: u64,
    frame: Option<Frame>,
    result: Result<Vec<claude::Suggestion>, String>,
}

fn hash_text(text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Run the checker loop until the event channel closes.
pub async fn run(app: AppHandle, mut rx: UnboundedReceiver<AxEvent>) {
    let client = reqwest::Client::new();
    let (done_tx, mut done_rx): (UnboundedSender<CheckDone>, UnboundedReceiver<CheckDone>) =
        tokio::sync::mpsc::unbounded_channel();

    let mut pending: Option<Pending> = None;
    let mut inflight: Option<JoinHandle<()>> = None;
    let mut last_checked_hash: Option<u64> = None;

    let debounce: Sleep = tokio::time::sleep(IDLE);
    tokio::pin!(debounce);

    loop {
        tokio::select! {
            maybe = rx.recv() => {
                match maybe {
                    None => break, // channel closed → shutting down
                    Some(AxEvent::Hide { session_id }) => {
                        log::debug!("[check] hide (session={session_id})");
                        pending = None;
                        if let Some(h) = inflight.take() { h.abort(); }
                        debounce.as_mut().reset(Instant::now() + IDLE);
                        panel::hide(&app);
                    }
                    Some(AxEvent::TextChanged { session_id, text, frame }) => {
                        // New text supersedes any in-flight check.
                        if let Some(h) = inflight.take() { h.abort(); }
                        pending = Some(Pending { session_id, text, frame });
                        debounce.as_mut().reset(Instant::now() + DEBOUNCE);
                    }
                }
            }

            _ = &mut debounce, if pending.is_some() => {
                debounce.as_mut().reset(Instant::now() + IDLE);
                let Pending { session_id, text, frame } = pending.take().unwrap();

                let h = hash_text(&text);
                if last_checked_hash == Some(h) {
                    continue; // identical to last check — nothing new to say
                }
                last_checked_hash = Some(h);

                let client = client.clone();
                let done_tx = done_tx.clone();
                inflight = Some(tokio::spawn(async move {
                    let result = claude::fetch_suggestions(&client, &text).await;
                    // If this task was aborted, the send simply never runs.
                    let _ = done_tx.send(CheckDone { session_id, frame, result });
                }));
            }

            Some(done) = done_rx.recv() => {
                inflight = None;
                match done.result {
                    Ok(suggestions) => {
                        log::info!(
                            "[check] session={} → {} suggestion(s)",
                            done.session_id,
                            suggestions.len()
                        );
                        panel::show(&app, done.frame, done.session_id, suggestions);
                    }
                    Err(e) => {
                        log::warn!("[check] session={} error: {}", done.session_id, e);
                    }
                }
            }
        }
    }
}
