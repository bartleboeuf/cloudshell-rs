/// Full CloudShell session: Create environment, session, and connect via session-manager-plugin.
/// This mirrors the Python example.py workflow with actual SSM connection.
///
/// Prerequisites:
///   - session-manager-plugin installed: https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html
///
/// Usage:
///   cargo run --example connect_via_ssm -- --profile your-profile --region us-east-1
use cloudshell_rs::CloudShellClient;
use cloudshell_rs::progress::StdoutProgress;
use cloudshell_rs::ssm::{check_session_manager_plugin, connect_via_ssm};
use cloudshell_rs::workflow::{get_or_create_env, wait_for_running};
use serde_json::json;
use std::time::Duration;
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
    let env_id = get_or_create_env(&client, &StdoutProgress).await?;
    println!("✅ Using environment: {}", env_id);

    // Step 2: Wait for environment to be running
    println!("\n⏳ Waiting for environment to be RUNNING...");
    wait_for_running(
        &client,
        &env_id,
        Duration::from_secs(args.timeout_secs),
        &StdoutProgress,
    )
    .await?;
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

    let connect_result = connect_via_ssm(
        &client,
        &env_id,
        &session_id,
        &payload.to_string(),
        &args.region,
        args.inject_credentials,
        &StdoutProgress,
    )
    .await;

    // Step 5: Cleanup, runs even if the connection failed
    println!("\n🧹 Cleaning up...");
    match client.delete_session(&env_id, &session_id).await {
        Ok(()) => println!("✅ Session deleted"),
        Err(err) => eprintln!("⚠️  Failed to delete session {}: {}", session_id, err),
    }
    connect_result?;

    println!("\n✨ CloudShell session complete!");
    println!("💡 Tip: Environment kept for reuse (call 'delete' to remove it)");

    Ok(())
}
