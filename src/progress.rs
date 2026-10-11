//! Progress reporting for long-running operations.
//!
//! The library never prints on its own. Callers choose where progress messages go by passing a
//! [`Progress`] implementation to functions such as [`crate::workflow::wait_for_running`].

/// Receives human-readable progress messages from library operations.
pub trait Progress: Send + Sync {
    /// Report one progress message.
    fn info(&self, message: &str);
}

/// Writes progress messages to standard output. Used by the CLI and the examples.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdoutProgress;

impl Progress for StdoutProgress {
    fn info(&self, message: &str) {
        println!("{message}");
    }
}

/// Discards all progress messages. Use this when embedding the library silently.
#[derive(Debug, Clone, Copy, Default)]
pub struct SilentProgress;

impl Progress for SilentProgress {
    fn info(&self, _message: &str) {}
}

#[cfg(test)]
mod tests {
    use super::{Progress, SilentProgress};

    #[test]
    fn silent_progress_accepts_messages_without_output() {
        SilentProgress.info("not printed");
    }
}
