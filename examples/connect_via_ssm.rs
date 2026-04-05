/// Full CloudShell session: Create environment, session, and connect via session-manager-plugin.
/// This mirrors the Python example.py workflow with actual SSM connection.
///
/// Prerequisites:
///   - session-manager-plugin installed: https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html
///
/// Usage:
///   cargo run --example connect_via_ssm -- --profile your-profile --region us-east-1
use cloudshell_rs::CloudShellClient;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use serde_json::json;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(clap::Parser)]
#[command(name = "connect_via_ssm")]
#[command(about = "Connect to CloudShell via SSM Session Manager")]
struct Args {
    #[arg(short, long)]
    profile: Option<String>,

    #[arg(short, long, default_value = "us-east-1")]
    region: String,

    #[arg(long, default_value = "120")]
    timeout_secs: u64,

    #[arg(long, default_value = "false")]
    inject_credentials: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use clap::Parser;

    let args = Args::parse();

    // Check for session-manager-plugin
    if !check_session_manager_plugin() {
        eprintln!("❌ session-manager-plugin not found in PATH");
        eprintln!(
            "   Install from: https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html"
        );
        return Err("session-manager-plugin is required".into());
    }
    println!("✅ session-manager-plugin found");

    println!(
        "\n🚀 Creating CloudShell client (region={})...",
        args.region
    );
    let client = CloudShellClient::new(args.region.clone(), args.profile).await?;

    // Step 1: Find or create environment
    println!("\n📋 Looking for existing environments...");
    let env_id = get_or_create_env(&client).await?;
    println!("✅ Using environment: {}", env_id);

    // Step 2: Wait for environment to be running
    println!("\n⏳ Waiting for environment to be RUNNING...");
    wait_for_running(&client, &env_id, Duration::from_secs(args.timeout_secs)).await?;
    println!("✅ Environment is ready!");

    // Step 3: Create a session
    println!("\n🔌 Creating session...");
    let tab_id = Uuid::new_v4().to_string();
    let session_resp = client
        .create_session(&env_id, "TMUX", &tab_id, Some(true))
        .await?;

    let session_id = session_resp
        .session_id()
        .ok_or("No session ID in response")?;
    let token_value = session_resp.token_value().ok_or("No token in response")?;
    let stream_url = session_resp
        .stream_url()
        .ok_or("No stream URL in response")?;

    println!("✅ Session created: {}", session_id);
    println!("📡 Stream URL: {}", stream_url);

    // Step 4: Connect via session-manager-plugin
    println!("\n🔗 Connecting via session-manager-plugin...");
    let payload = json!({
        "SessionId": session_id,
        "TokenValue": token_value,
        "StreamUrl": stream_url,
    });

    connect_via_ssm(
        &client,
        &env_id,
        &session_id,
        &payload.to_string(),
        &args.region,
        args.inject_credentials,
    )
    .await?;

    // Step 5: Cleanup
    println!("\n🧹 Cleaning up...");
    client.delete_session(&env_id, &session_id).await?;
    println!("✅ Session deleted");

    println!("\n✨ CloudShell session complete!");
    println!("💡 Tip: Environment kept for reuse (call 'delete' to remove it)");

    Ok(())
}

/// Check if session-manager-plugin is available
fn check_session_manager_plugin() -> bool {
    Command::new("session-manager-plugin")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Connect to CloudShell environment via session-manager-plugin
async fn connect_via_ssm(
    client: &CloudShellClient,
    env_id: &str,
    session_id: &str,
    payload: &str,
    region: &str,
    inject_credentials: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if inject_credentials {
        // Inject credentials mode: set env vars, verify with test commands, then go interactive
        // Enable raw mode for proper terminal handling (backspace, arrow keys, etc.)
        enable_raw_mode()?;

        let result = {
            let mut child = Command::new("session-manager-plugin")
                .arg(payload)
                .arg(region)
                .arg("StartSession")
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?;

            let mut stdin = child.stdin.take().ok_or("Failed to open stdin")?;

            // Wait for shell to be ready
            println!("⏳ Waiting for shell to be ready...");
            sleep(Duration::from_secs(3)).await;

            // Disable echo to hide credential exports from screen
            writeln!(stdin, "stty -echo")?;
            sleep(Duration::from_millis(500)).await;

            // Get current credentials from the client
            let creds = client.get_credentials();
            let access_key = creds.access_key_id();
            let secret_key = creds.secret_access_key();
            let session_token = creds.session_token();

            // Build commands: secure credential injection
            let mut commands = vec![
                " set +o history".to_string(),
                " export HISTFILE=/dev/null".to_string(),
                " export HISTCONTROL=ignorespace".to_string(),
                format!(" export AWS_ACCESS_KEY_ID={}", access_key),
                format!(" export AWS_SECRET_ACCESS_KEY={}", secret_key),
            ];

            if let Some(token) = session_token {
                commands.push(format!(" export AWS_SESSION_TOKEN={}", token));
            }

            commands.extend(vec![
                format!(" export AWS_DEFAULT_REGION={}", region),
                " readonly AWS_ACCESS_KEY_ID".to_string(),
                " readonly AWS_SECRET_ACCESS_KEY".to_string(),
                " readonly AWS_SESSION_TOKEN".to_string(),
                " clear".to_string(),
                " set -o history".to_string(),
            ]);

            // Send credentials without displaying them
            for cmd in &commands {
                writeln!(stdin, "{}", cmd)?;
                sleep(Duration::from_millis(500)).await;
            }

            // Re-enable echo and confirm completion
            writeln!(stdin, "stty echo")?;
            sleep(Duration::from_millis(500)).await;

            println!("✅ Connected! Shell is open (type 'exit' to disconnect)");

            // Forward user input to subprocess as raw bytes (not line-buffered)
            thread::spawn(move || {
                let mut buf = [0u8; 256];
                let stdin_fd = std::io::stdin();
                let mut stdin_lock = stdin_fd.lock();
                loop {
                    match stdin_lock.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let _ = stdin.write_all(&buf[..n]);
                            let _ = stdin.flush();
                        }
                        Err(_) => break,
                    }
                }
            });

            // Wait for user to finish
            child.wait()?
        };

        // Always restore terminal to normal mode
        disable_raw_mode()?;

        if !result.success() {
            return Err("session-manager-plugin exited with error".into());
        }
    } else {
        // Interactive mode: inherit stdin from parent process (keeps it open)
        println!("⏳ Waiting for shell to be ready...");
        sleep(Duration::from_secs(3)).await;

        println!("🔓 Interactive shell ready!");
        println!("💡 Type 'exit' to disconnect");
        println!("📝 Session Info:");
        println!("   Environment ID: {}", env_id);
        println!("   Session ID: {}", session_id);

        // Enable raw mode for proper terminal handling (backspace, arrow keys, etc.)
        enable_raw_mode()?;
        let result = {
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
        // Always restore terminal to normal mode
        disable_raw_mode()?;

        if !result.success() {
            return Err("session-manager-plugin exited with error".into());
        }
    }

    Ok(())
}

/// Get existing environment or create a new one
async fn get_or_create_env(
    client: &CloudShellClient,
) -> Result<String, Box<dyn std::error::Error>> {
    let envs = client.describe_environments().await?.environments();

    if !envs.is_empty() {
        let env_id = envs[0]
            .get("EnvironmentId")
            .and_then(|id| id.as_str())
            .ok_or("No EnvironmentId in response")?;

        println!("📍 Found existing environment: {}", env_id);
        return Ok(env_id.to_string());
    }

    println!("✨ No environment found, creating...");
    let create_resp = client.create_environment(None, None).await?;
    let env_id = create_resp
        .environment_id()
        .ok_or("No environment ID in create response")?;

    println!("✅ Created environment: {}", env_id);
    Ok(env_id)
}

/// Wait for environment to reach RUNNING state
async fn wait_for_running(
    client: &CloudShellClient,
    env_id: &str,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();

    loop {
        let status_resp = client.get_environment_status(env_id).await?;
        let status = status_resp.status().ok_or("No status in response")?;

        if status == "RUNNING" {
            return Ok(());
        }

        // Start suspended environment
        if status == "SUSPENDED" {
            println!("🔄 Environment is SUSPENDED, starting...");
            client.start_environment(env_id).await?;
        }

        // Check timeout
        if start.elapsed() > timeout {
            return Err(format!("Timeout: Environment stuck in {} state", status).into());
        }

        println!("⏳ Status: {} - waiting...", status);
        sleep(Duration::from_secs(3)).await;
    }
}
