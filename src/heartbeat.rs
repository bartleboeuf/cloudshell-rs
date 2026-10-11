//! Background heartbeat helper that keeps a CloudShell environment from suspending.

use crate::{CloudShellClient, CloudShellError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

/// Handle to a background heartbeat task.
///
/// The heartbeat stops when this handle is dropped or [`Heartbeat::stop`] is called.
pub struct Heartbeat {
    handle: JoinHandle<()>,
    beats: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<CloudShellError>>>,
}

impl Heartbeat {
    /// Number of heartbeats successfully sent so far.
    pub fn beats_sent(&self) -> u64 {
        self.beats.load(Ordering::Relaxed)
    }

    /// Error from the most recent heartbeat attempt, if it failed. Cleared on the next success.
    pub fn last_error(&self) -> Option<String> {
        self.last_error
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|err| err.to_string()))
    }

    /// Whether the background task is still running.
    pub fn is_running(&self) -> bool {
        !self.handle.is_finished()
    }

    /// Stop the heartbeat task.
    pub fn stop(self) {
        // Dropping `self` aborts the task.
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Spawn a task that sends a heartbeat immediately, then every `interval`.
///
/// The interval is clamped to at least one second. Failures do not stop the task;
/// they are recorded and exposed through [`Heartbeat::last_error`], and the next
/// attempt happens on the following tick.
///
/// # Example
/// ```ignore
/// use std::{sync::Arc, time::Duration};
/// let client = Arc::new(CloudShellClient::new("us-east-1".into(), None).await?);
/// let hb = spawn_heartbeat(Arc::clone(&client), env_id, Duration::from_secs(300));
/// // ... long-running work ...
/// hb.stop();
/// ```
pub fn spawn_heartbeat(
    client: Arc<CloudShellClient>,
    environment_id: impl Into<String>,
    interval: Duration,
) -> Heartbeat {
    let environment_id = environment_id.into();
    let interval = interval.max(Duration::from_secs(1));
    let beats = Arc::new(AtomicU64::new(0));
    let last_error = Arc::new(Mutex::new(None::<CloudShellError>));

    let handle = tokio::spawn({
        let beats = Arc::clone(&beats);
        let last_error = Arc::clone(&last_error);
        async move {
            loop {
                match client.send_heart_beat(&environment_id).await {
                    Ok(()) => {
                        beats.fetch_add(1, Ordering::Relaxed);
                        if let Ok(mut guard) = last_error.lock() {
                            *guard = None;
                        }
                    }
                    Err(err) => {
                        if let Ok(mut guard) = last_error.lock() {
                            *guard = Some(err);
                        }
                    }
                }
                tokio::time::sleep(interval).await;
            }
        }
    });

    Heartbeat {
        handle,
        beats,
        last_error,
    }
}
