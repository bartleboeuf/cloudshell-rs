/// Full lifecycle example: Create/get environment, wait for ready, create session, cleanup.
/// This mirrors the Python example.py workflow using the cloudshell_rs library.
///
/// Usage:
///   cargo run --example full_lifecycle -- --profile your-profile --region us-east-1
use cloudshell_rs::CloudShellClient;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(clap::Parser)]
#[command(name = "full_lifecycle")]
#[command(about = "Full CloudShell environment lifecycle")]
struct Args {
    #[arg(short, long)]
    profile: Option<String>,

    #[arg(short, long, default_value = "us-east-1")]
    region: String,

    #[arg(long, default_value = "120")]
    timeout_secs: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use clap::Parser;

    let args = Args::parse();

    println!("🚀 Creating CloudShell client (region={})...", args.region);
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
    let _token = session_resp.token_value().ok_or("No token in response")?;
    let stream_url = session_resp
        .stream_url()
        .ok_or("No stream URL in response")?;

    println!("✅ Session created: {}", session_id);
    println!("📡 Stream URL: {}", stream_url);
    println!("🔐 Token: (hidden for security)");

    // Step 4: Print session info (in production, you'd use session-manager-plugin)
    println!("\n📋 Session Information:");
    println!("{}", session_resp.pretty_print()?);

    // Step 5: Send heartbeat to keep alive
    println!("\n💓 Sending heartbeat to keep environment alive...");
    client.send_heart_beat(&env_id).await?;
    println!("✅ Heartbeat sent");

    // Step 6: File transfer - Upload
    println!("\n📤 File Upload Demo...");
    let upload_resp = client.get_file_upload_urls(&env_id).await?;

    if let Some(upload_url) = upload_resp.file_upload_presigned_url() {
        println!("✅ Got upload URL: {}", upload_url);

        // Show presigned fields
        if let Some(fields) = upload_resp.file_upload_presigned_fields() {
            println!("📋 Presigned Fields:");
            println!(
                "   Key: {}",
                fields.get("key").and_then(|k| k.as_str()).unwrap_or("N/A")
            );
            println!(
                "   Bucket: {}",
                fields
                    .get("bucket")
                    .and_then(|b| b.as_str())
                    .unwrap_or("N/A")
            );
            println!(
                "   Algorithm: {}",
                fields
                    .get("X-Amz-Algorithm")
                    .and_then(|a| a.as_str())
                    .unwrap_or("N/A")
            );
        }

        // In production, you would use these to upload via:
        // requests.post(upload_url, data=presigned_fields, files={'file': ('example.txt', file_content)})
        println!("💡 In production, use these presigned URLs with HTTP client to upload files");
        println!(
            "   Example: curl -F key=<value> -F file=@example.txt {}",
            upload_url
        );
    } else {
        println!("⚠️  No upload URL available");
    }

    // Step 7: File transfer - Download
    println!("\n📥 File Download Demo...");
    let download_resp = client.get_file_download_urls(&env_id).await?;

    if let Some(download_url) = download_resp.file_download_presigned_url() {
        println!("✅ Got download URL: {}", download_url);

        if let Some(key) = download_resp.file_download_presigned_key() {
            println!("📋 Download Key: {}", key);
        }
        if let Some(hash) = download_resp.file_download_presigned_key_hash() {
            println!("🔒 Key Hash: {}", hash);
        }

        // In production, you would download via:
        // response = requests.get(download_url, headers={'x-amz-server-side-encryption-customer-key': key})
        println!("💡 In production, use this presigned URL with HTTP client to download files");
        println!("   Example: curl -o downloaded_file {}", download_url);
    } else {
        println!("⚠️  No download URL available");
    }

    // Step 8: Cleanup
    println!("\n🧹 Cleaning up...");
    client.delete_session(&env_id, &session_id).await?;
    println!("✅ Session deleted");

    // Step 9: Optional - Send final heartbeat
    println!("\n💓 Sending final heartbeat...");
    client.send_heart_beat(&env_id).await?;
    println!("✅ Environment kept alive for reuse");

    println!("\n✨ Environment lifecycle complete!");
    println!("💡 Tip: Environment kept for reuse (call 'delete' to remove it)");

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
