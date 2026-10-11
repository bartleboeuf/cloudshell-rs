//! Interactive shell access through `session-manager-plugin`.
//!
//! Shared by the CLI (`connect` command) and the examples so the SSM logic lives in one place.

use crate::progress::Progress;
use crate::{CloudResult, CloudShellClient, CloudShellError};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{Read, Write};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// Marker printed by the remote shell once the credential setup has run.
const READY_MARKER: &str = "__CS_READY__";

/// Returns `true` if `session-manager-plugin` is available on the PATH.
pub fn check_session_manager_plugin() -> bool {
    Command::new("session-manager-plugin")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Connect to a CloudShell environment via `session-manager-plugin`.
///
/// With `inject_credentials`, the current credentials are loaded into the remote bash shell
/// (see [`send_credentials_to_shell`]). Otherwise the plugin is attached to the terminal directly.
/// Progress messages are sent to `progress`; the plugin's own shell output is always shown.
pub async fn connect_via_ssm(
    client: &CloudShellClient,
    env_id: &str,
    session_id: &str,
    payload: &str,
    region: &str,
    inject_credentials: bool,
    progress: &dyn Progress,
) -> CloudResult<()> {
    if inject_credentials {
        progress.info(
            "💉 Injecting AWS credentials (not echoed; temporary file removed after load)...",
        );

        let mut child = Command::new("session-manager-plugin")
            .arg(payload)
            .arg(region)
            .arg("StartSession")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let mut stdin = child.stdin.take().ok_or_else(|| {
            CloudShellError::Process("Failed to open session-manager-plugin stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            CloudShellError::Process("Failed to open session-manager-plugin stdout".to_string())
        })?;

        let (ready_tx, ready_rx) = mpsc::channel();
        let relay = spawn_output_relay(stdout, ready_tx);

        let status = {
            // Raw mode for proper terminal handling; restored when this block ends
            let _raw_mode = RawModeGuard::enable()?;

            if let Err(err) = send_credentials_to_shell(&mut stdin, ready_rx, client, region).await
            {
                let _ = child.kill();
                let _ = child.wait();
                let _ = relay.join();
                return Err(err);
            }

            // Forward user input to the plugin as raw bytes (not line-buffered)
            thread::spawn(move || {
                let mut buf = [0u8; 256];
                let stdin_handle = std::io::stdin();
                let mut stdin_lock = stdin_handle.lock();
                loop {
                    match stdin_lock.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let _ = stdin.write_all(&buf[..n]);
                            let _ = stdin.flush();
                        }
                    }
                }
            });

            // Wait for user to finish
            child.wait()?
        };
        let _ = relay.join();

        if !status.success() {
            return Err(CloudShellError::Process(
                "session-manager-plugin exited with error".to_string(),
            ));
        }
    } else {
        // Interactive mode: inherit stdin from parent process (keeps it open)
        progress.info("⏳ Waiting for shell to be ready...");
        sleep(Duration::from_secs(3)).await;

        progress.info("🔓 Interactive shell ready!");
        progress.info("💡 Type 'exit' to disconnect");
        progress.info("📝 Session Info:");
        progress.info(&format!("   Environment ID: {env_id}"));
        progress.info(&format!("   Session ID: {session_id}"));

        let result = {
            // Raw mode for proper terminal handling; restored when this block ends
            let _raw_mode = RawModeGuard::enable()?;

            let mut child = Command::new("session-manager-plugin")
                .arg(payload)
                .arg(region)
                .arg("StartSession")
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?;

            // Wait indefinitely for subprocess (user will exit shell manually)
            child.wait()?
        };

        if !result.success() {
            return Err(CloudShellError::Process(
                "session-manager-plugin exited with error".to_string(),
            ));
        }
    }

    Ok(())
}

/// Restores the terminal to cooked mode when dropped, including on early return or error.
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> std::io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

/// Quote a value as a single POSIX shell word.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Relay the plugin's stdout to the local terminal and signal once the ready marker appears.
fn spawn_output_relay(mut stdout: ChildStdout, ready: mpsc::Sender<()>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut out = std::io::stdout();
        let mut buf = [0u8; 1024];
        let mut tail = String::new();
        let mut signalled = false;
        loop {
            let n = match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let _ = out.write_all(&buf[..n]);
            let _ = out.flush();
            if !signalled {
                tail.push_str(&String::from_utf8_lossy(&buf[..n]));
                if tail.contains(READY_MARKER) {
                    signalled = true;
                    let _ = ready.send(());
                }
                // Keep only the last 256 characters so the buffer stays small
                if tail.len() > 4096 {
                    let keep_from = tail
                        .char_indices()
                        .rev()
                        .nth(255)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    tail.drain(..keep_from);
                }
            }
        }
    })
}

/// Load AWS credentials into the remote bash shell.
///
/// The values are written to a mode-600 file in tmpfs (`/dev/shm`), sourced, and removed
/// immediately. History and echo are disabled while the secrets are typed, and a `trap`
/// unsets the variables when the shell exits. Requires bash on the CloudShell side.
async fn send_credentials_to_shell(
    stdin: &mut ChildStdin,
    ready: mpsc::Receiver<()>,
    client: &CloudShellClient,
    region: &str,
) -> CloudResult<()> {
    let creds = client.get_credentials();
    let cred_file = format!("/dev/shm/cloudshell-rs-{}", Uuid::new_v4());

    let mut setup = vec![
        "stty -echo".to_string(),
        "set +o history".to_string(),
        format!("umask 077; f={}; : > \"$f\"", shell_quote(&cred_file)),
        format!(
            "printf 'export %s=%q\\n' AWS_ACCESS_KEY_ID {} >> \"$f\"",
            shell_quote(creds.access_key_id())
        ),
        format!(
            "printf 'export %s=%q\\n' AWS_SECRET_ACCESS_KEY {} >> \"$f\"",
            shell_quote(creds.secret_access_key())
        ),
    ];
    if let Some(token) = creds.session_token() {
        setup.push(format!(
            "printf 'export %s=%q\\n' AWS_SESSION_TOKEN {} >> \"$f\"",
            shell_quote(token)
        ));
    }
    setup.extend([
        format!(
            "printf 'export %s=%q\\n' AWS_DEFAULT_REGION {} >> \"$f\"",
            shell_quote(region)
        ),
        ". \"$f\"; rm -f \"$f\"; unset f".to_string(),
        "set -o history".to_string(),
        "trap 'unset AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN' EXIT".to_string(),
        "stty echo; clear".to_string(),
        "printf '__CS_%s__\\n' READY".to_string(),
    ]);

    for line in &setup {
        writeln!(stdin, "{line}")?;
    }
    stdin.flush()?;

    // Wait for the remote shell to confirm the setup ran, instead of guessing with sleeps
    let confirmed =
        tokio::task::spawn_blocking(move || ready.recv_timeout(Duration::from_secs(30)))
            .await
            .map_err(|err| CloudShellError::Process(err.to_string()))?;
    if confirmed.is_err() {
        return Err(CloudShellError::Timeout(
            "Shell did not confirm credential setup".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::shell_quote;

    #[test]
    fn shell_quote_neutralizes_metacharacters() {
        assert_eq!(shell_quote("abc"), "'abc'");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
        assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
    }
}
