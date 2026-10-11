use clap::{Parser, Subcommand};
use cloudshell_rs::progress::StdoutProgress;
use cloudshell_rs::ssm::{check_session_manager_plugin, connect_via_ssm};
use cloudshell_rs::workflow::{get_or_create_env, wait_for_running};
use cloudshell_rs::{CloudResult, CloudShellClient, CloudShellError, VpcConfig};
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "cloudshell-rs")]
#[command(about = "AWS CloudShell API client")]
struct Args {
    /// AWS profile to use
    #[arg(short, long)]
    profile: Option<String>,

    /// AWS region
    #[arg(short, long, default_value = "us-east-1")]
    region: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List all CloudShell environments in the region
    ///
    /// Shows all environments with their IDs, status, and other details.
    /// Output is pretty-printed JSON.
    Describe,

    /// Create a new CloudShell environment (public or VPC)
    ///
    /// Creates either a public environment (default) or a VPC environment.
    ///
    /// **Public environment:**
    /// - No additional options required
    /// - Access from AWS Console or CLI tools
    ///
    /// **VPC environment:**
    /// - Requires: --vpc-id, --subnet-ids, --security-group-ids
    /// - Max 5 security groups allowed
    /// - Min 1 subnet required
    ///
    /// **Example:**
    /// ```
    /// # Public
    /// cloudshell-rs create
    /// cloudshell-rs create --name my-env
    ///
    /// # VPC
    /// cloudshell-rs create --name my-vpc-env \
    ///   --vpc-id vpc-123 \
    ///   --subnet-ids subnet-1,subnet-2 \
    ///   --security-group-ids sg-1,sg-2
    /// ```
    #[command(about = "Create a new CloudShell environment (public or VPC)")]
    Create {
        /// Environment name (optional for public, required for VPC)
        #[arg(long)]
        name: Option<String>,

        /// VPC ID (optional - if set, creates VPC environment)
        #[arg(long)]
        vpc_id: Option<String>,

        /// Subnet IDs (comma-separated, required if vpc_id is set, min 1)
        #[arg(long)]
        subnet_ids: Option<String>,

        /// Security group IDs (comma-separated, required if vpc_id is set, max 5)
        #[arg(long)]
        security_group_ids: Option<String>,
    },

    /// Get the current status of an environment
    ///
    /// Returns the environment status (CREATING, RUNNING, SUSPENDED, PENDING, DELETING)
    /// and optional status reason.
    ///
    /// **Example:**
    /// ```
    /// cloudshell-rs status abc123-def4-5678-ghij
    /// ```
    #[command(about = "Get environment status (CREATING, RUNNING, SUSPENDED, etc)")]
    Status {
        /// Environment ID (use 'describe' to list all)
        #[arg(value_name = "ENV_ID")]
        environment_id: String,
    },

    /// Start a suspended CloudShell environment
    ///
    /// Transitions the environment from SUSPENDED state to RUNNING state.
    /// No-op if already running.
    ///
    /// **Example:**
    /// ```
    /// cloudshell-rs start abc123-def4-5678-ghij
    /// ```
    #[command(about = "Start a suspended environment")]
    Start {
        /// Environment ID to start
        #[arg(value_name = "ENV_ID")]
        environment_id: String,
    },

    /// Stop a running CloudShell environment
    ///
    /// Transitions the environment to SUSPENDED state.
    /// Can be started again with the 'start' command.
    /// Use 'delete' to permanently remove it.
    ///
    /// **Example:**
    /// ```
    /// cloudshell-rs stop abc123-def4-5678-ghij
    /// ```
    #[command(about = "Stop a running environment (transitions to SUSPENDED)")]
    Stop {
        /// Environment ID to stop
        #[arg(value_name = "ENV_ID")]
        environment_id: String,
    },

    /// Delete a CloudShell environment
    ///
    /// Permanently removes the environment. The environment must be in
    /// RUNNING or SUSPENDED state. Cannot be undone.
    ///
    /// **Warning:** This is irreversible. All data in the environment will be lost.
    ///
    /// **Example:**
    /// ```
    /// cloudshell-rs delete abc123-def4-5678-ghij
    /// ```
    #[command(about = "Delete an environment (⚠️  irreversible)")]
    Delete {
        /// Environment ID to delete
        #[arg(value_name = "ENV_ID")]
        environment_id: String,
    },

    /// Create an interactive session to an environment
    ///
    /// Creates an SSM WebSocket session that can be used with session-manager-plugin
    /// to get an interactive shell. Returns SessionId, TokenValue, and StreamUrl.
    ///
    /// Use the 'connect' command for a simpler interactive shell experience.
    ///
    /// **Note:** TabId must be a valid UUID v4.
    ///
    /// **Example:**
    /// ```
    /// cloudshell-rs session abc123-def4-5678-ghij --tab-id $(uuidgen)
    /// ```
    #[command(about = "Create a session (returns SessionId, TokenValue, StreamUrl)")]
    Session {
        /// Environment ID to connect to
        #[arg(value_name = "ENV_ID")]
        environment_id: String,

        /// Session type (usually 'TMUX')
        #[arg(long, default_value = "TMUX")]
        r#type: String,

        /// Tab ID - must be a valid UUID v4 (use 'uuidgen' to generate)
        #[arg(long)]
        tab_id: String,
    },

    /// Send a heartbeat to keep an environment alive
    ///
    /// CloudShell environments go to sleep after ~20 minutes of inactivity.
    /// Sending heartbeats periodically prevents the environment from suspending.
    ///
    /// Useful for long-running operations or batch jobs.
    ///
    /// **Example:**
    /// ```
    /// # Send heartbeat once
    /// cloudshell-rs heartbeat abc123-def4-5678-ghij
    ///
    /// # Keep alive every 5 minutes
    /// while true; do
    ///   cloudshell-rs heartbeat abc123-def4-5678-ghij
    ///   sleep 300
    /// done
    /// ```
    #[command(about = "Send heartbeat to keep environment alive (prevents auto-sleep)")]
    Heartbeat {
        /// Environment ID
        #[arg(value_name = "ENV_ID")]
        environment_id: String,
    },

    /// Get S3 presigned URLs for uploading files
    ///
    /// Returns a presigned URL and form fields for uploading a file to the environment.
    /// Use with 'curl' or 'requests' library to upload files.
    ///
    /// Files are placed in the environment's home directory.
    ///
    /// **Example:**
    /// ```
    /// # Get upload URL
    /// cloudshell-rs upload abc123-def4-5678-ghij
    ///
    /// # Upload a file (extract URL and fields from response)
    /// curl -F file=@script.sh \
    ///   -F key=<key> -F bucket=<bucket> \
    ///   -F X-Amz-Algorithm=AWS4-HMAC-SHA256 \
    ///   ... <presigned_url>
    /// ```
    #[command(about = "Get presigned S3 URLs for file upload")]
    Upload {
        /// Environment ID
        #[arg(value_name = "ENV_ID")]
        environment_id: String,
    },

    /// Get S3 presigned URLs for downloading files
    ///
    /// Returns a presigned URL to download files from the environment.
    /// Use with 'curl' or 'requests' library to download files.
    ///
    /// Also returns a decryption key and key hash for downloading encrypted files.
    ///
    /// **Example:**
    /// ```
    /// cloudshell-rs download abc123-def4-5678-ghij
    /// curl <presigned_url> -o myfile.txt
    /// ```
    #[command(about = "Get presigned S3 URLs for file download")]
    Download {
        /// Environment ID
        #[arg(value_name = "ENV_ID")]
        environment_id: String,
    },

    /// Connect to CloudShell via SSM Session Manager (interactive shell)
    ///
    /// Provides an interactive shell to a CloudShell environment.
    /// Automatically handles environment creation, startup, and session management.
    ///
    /// **Modes:**
    /// 1. **Interactive shell (default)**: Get full shell access via session-manager-plugin
    /// 2. **Credential injection (--inject-credentials)**: Auto-inject AWS credentials silently
    ///
    /// **Prerequisites:**
    /// - session-manager-plugin must be installed
    /// - AWS credentials must be configured (profile or env vars)
    ///
    /// **Security notes for credential injection:**
    /// - Terminal echo disabled during injection (credentials never shown on screen)
    /// - Bash history disabled (HISTCONTROL=ignorespace, set +o history)
    /// - Credentials marked as readonly to prevent accidental exposure
    /// - Screen cleared after injection to remove visual traces
    ///
    /// **Example:**
    /// ```
    /// # Interactive shell (default)
    /// cloudshell-rs connect
    /// cloudshell-rs connect abc123-def4-5678-ghij
    /// cloudshell-rs connect --timeout-secs 300
    ///
    /// # Auto-inject credentials (silent, secure)
    /// cloudshell-rs connect --inject-credentials
    /// cloudshell-rs connect --profile myprofile --inject-credentials
    /// ```
    #[command(about = "Connect to CloudShell via SSM Session Manager (interactive shell)")]
    Connect {
        /// Environment ID (optional - uses existing or creates new)
        #[arg(value_name = "ENV_ID")]
        environment_id: Option<String>,

        /// Max seconds to wait for environment to be ready (default: 120)
        #[arg(long, default_value = "120")]
        timeout_secs: u64,

        /// Inject AWS credentials silently (hidden from screen and bash history)
        #[arg(long, default_value = "false")]
        inject_credentials: bool,
    },
}

#[tokio::main]
async fn main() -> CloudResult<()> {
    let args = Args::parse();

    // Create CloudShell client (clone region for later use in connect command)
    let client = CloudShellClient::new(args.region.clone(), args.profile).await?;

    match args.command.unwrap_or(Command::Describe) {
        Command::Describe => {
            let response = client.describe_environments().await?;
            println!("{}", response.pretty_print()?);
        }

        Command::Create {
            name,
            vpc_id,
            subnet_ids,
            security_group_ids,
        } => {
            // Parse VPC configuration if provided
            let vpc_config = if let Some(vpc_id) = vpc_id {
                // VPC environment: require subnet and security groups
                let subnets = subnet_ids
                    .ok_or_else(|| {
                        CloudShellError::Validation(
                            "--subnet-ids required when --vpc-id is provided".to_string(),
                        )
                    })?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();

                let security_groups = security_group_ids
                    .ok_or_else(|| {
                        CloudShellError::Validation(
                            "--security-group-ids required when --vpc-id is provided".to_string(),
                        )
                    })?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();

                // Validate: at least 1 subnet, max 5 security groups
                if subnets.is_empty() {
                    return Err(CloudShellError::Validation(
                        "At least 1 subnet is required".to_string(),
                    ));
                }
                if security_groups.is_empty() {
                    return Err(CloudShellError::Validation(
                        "At least 1 security group is required".to_string(),
                    ));
                }
                if security_groups.len() > 5 {
                    return Err(CloudShellError::Validation(format!(
                        "Max 5 security groups allowed, got {}",
                        security_groups.len()
                    )));
                }

                Some(VpcConfig::new(vpc_id, subnets, security_groups))
            } else {
                // Public environment: no VPC config needed
                if subnet_ids.is_some() || security_group_ids.is_some() {
                    eprintln!(
                        "⚠️  Warning: --subnet-ids and --security-group-ids ignored (--vpc-id not set)"
                    );
                }
                None
            };

            let response = client
                .create_environment(name.as_deref(), vpc_config)
                .await?;
            println!("{}", response.pretty_print()?);
        }

        Command::Status { environment_id } => {
            let response = client.get_environment_status(&environment_id).await?;
            println!("{}", response.pretty_print()?);
        }

        Command::Start { environment_id } => {
            client.start_environment(&environment_id).await?;
            println!("Environment started: {}", environment_id);
        }

        Command::Stop { environment_id } => {
            let response = client.stop_environment(&environment_id).await?;
            println!("{}", response.pretty_print()?);
        }

        Command::Delete { environment_id } => {
            client.delete_environment(&environment_id).await?;
            println!("Environment deleted: {}", environment_id);
        }

        Command::Session {
            environment_id,
            r#type,
            tab_id,
        } => {
            let response = client
                .create_session(&environment_id, &r#type, &tab_id, Some(true))
                .await?;
            println!("{}", response.pretty_print()?);
        }

        Command::Heartbeat { environment_id } => {
            client.send_heart_beat(&environment_id).await?;
            println!("Heartbeat sent to: {}", environment_id);
        }

        Command::Upload { environment_id } => {
            let response = client.get_file_upload_urls(&environment_id).await?;
            println!("{}", response.pretty_print()?);
        }

        Command::Download { environment_id } => {
            let response = client.get_file_download_urls(&environment_id).await?;
            println!("{}", response.pretty_print()?);
        }

        Command::Connect {
            environment_id,
            timeout_secs,
            inject_credentials,
        } => {
            // Check for session-manager-plugin
            if !check_session_manager_plugin() {
                eprintln!("❌ session-manager-plugin not found in PATH");
                eprintln!(
                    "   Install from: https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html"
                );
                return Err(CloudShellError::Process(
                    "session-manager-plugin is required".to_string(),
                ));
            }
            println!("✅ session-manager-plugin found");

            println!("\n🚀 CloudShell SSM Session Manager");

            // Step 1: Use provided env_id or find/create environment
            let env_id = if let Some(id) = environment_id {
                println!("📍 Using environment: {}", id);
                id
            } else {
                println!("\n📋 Looking for existing environments...");
                get_or_create_env(&client, &StdoutProgress).await?
            };
            println!("✅ Environment: {}", env_id);

            // Step 2: Wait for environment to be running
            println!("\n⏳ Waiting for environment to be RUNNING...");
            wait_for_running(
                &client,
                &env_id,
                Duration::from_secs(timeout_secs),
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

            let session_id = session_resp.session_id().ok_or_else(|| {
                CloudShellError::Response("No session ID in response".to_string())
            })?;
            let token_value = session_resp
                .token_value()
                .ok_or_else(|| CloudShellError::Response("No token in response".to_string()))?;
            let stream_url = session_resp.stream_url().ok_or_else(|| {
                CloudShellError::Response("No stream URL in response".to_string())
            })?;

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
                inject_credentials,
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
            println!("💡 Tip: Environment kept for reuse (use 'delete' command to remove it)");
        }
    }

    Ok(())
}
