//! Environment lookup and readiness helpers shared by the CLI and the examples.

use crate::progress::Progress;
use crate::{CloudResult, CloudShellClient, CloudShellError};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Return the first existing environment ID, or create a public environment if none exists.
pub async fn get_or_create_env(
    client: &CloudShellClient,
    progress: &dyn Progress,
) -> CloudResult<String> {
    let envs = client.describe_environments().await?.environments();

    if !envs.is_empty() {
        let env_id = envs[0]
            .get("EnvironmentId")
            .and_then(|id| id.as_str())
            .ok_or_else(|| CloudShellError::Response("No EnvironmentId in response".to_string()))?;

        progress.info(&format!("📍 Found existing environment: {env_id}"));
        return Ok(env_id.to_string());
    }

    progress.info("✨ No environment found, creating...");
    let create_resp = client.create_environment(None, None).await?;
    let env_id = create_resp.environment_id().ok_or_else(|| {
        CloudShellError::Response("No environment ID in create response".to_string())
    })?;

    progress.info(&format!("✅ Created environment: {env_id}"));
    Ok(env_id)
}

/// Poll until the environment is RUNNING, starting it if it is SUSPENDED.
///
/// Returns [`CloudShellError::Timeout`] if the environment is not running within `timeout`.
pub async fn wait_for_running(
    client: &CloudShellClient,
    env_id: &str,
    timeout: Duration,
    progress: &dyn Progress,
) -> CloudResult<()> {
    let start = Instant::now();

    loop {
        let status_resp = client.get_environment_status(env_id).await?;
        let status = status_resp
            .status()
            .ok_or_else(|| CloudShellError::Response("No status in response".to_string()))?;

        if status == "RUNNING" {
            return Ok(());
        }

        // Start suspended environment
        if status == "SUSPENDED" {
            progress.info("🔄 Environment is SUSPENDED, starting...");
            client.start_environment(env_id).await?;
        }

        // Check timeout
        if start.elapsed() > timeout {
            return Err(CloudShellError::Timeout(format!(
                "Timed out waiting for environment in {} state",
                status
            )));
        }

        progress.info(&format!("⏳ Status: {status} - waiting..."));
        sleep(Duration::from_secs(3)).await;
    }
}
