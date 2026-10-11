# cloudshell-rs

[![Release](https://github.com/bartleboeuf/cloudshell-rs/actions/workflows/release.yml/badge.svg)](https://github.com/bartleboeuf/cloudshell-rs/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> **Rust library and CLI** for the AWS CloudShell API — reverse-engineered from the AWS Console.

AWS CloudShell has no public SDK. This project provides a **type-safe Rust client** using manual SigV4 signing. It's a modern alternative to the Python [cloudshell-boto3](https://github.com/guyon-it-consulting/cloudshell-boto3) project.

> ⚠️ **Disclaimer** — This uses an undocumented API. AWS can change or break it at any time without notice. Do not build mission-critical systems on top of it.

---

## Features

✅ **All 12 CloudShell Operations**
- `DescribeEnvironments` — List environments
- `CreateEnvironment` — Create public or VPC environments
- `StartEnvironment` / `StopEnvironment` — Manage environment lifecycle
- `GetEnvironmentStatus` — Poll environment status
- `CreateSession` — Create SSM WebSocket sessions
- `SendHeartBeat` — Keep environments alive
- `DeleteSession` / `DeleteEnvironment` — Cleanup
- `GetFileUploadUrls` / `GetFileDownloadUrls` — File transfer
- `PutCredentials` — Forward credentials (console sessions only)

✨ **Type-Safe Rust Library**
- Compile-time type safety with a typed error enum (`CloudShellError`) and `CloudResult<T>`
- Response wrapper types with helper methods
- No runtime type errors
- Reusable across projects

🚀 **Native Binary**
- Single compiled binary; no Python runtime required
- Async I/O on Tokio
- Request timeouts and bounded retries for read-only calls

📦 **Dual Interface**
- Library: `CloudShellClient` for embedding in Rust apps
- CLI: `cloudshell-rs` binary with 10+ subcommands
- Example programs (full lifecycle, SSM integration)

🧪 **Tested**
- Unit and integration tests (69 passing, 3 ignored at last run)
- Input validation (VPC, UUID v4), shell-quoting and retry-policy checks
- Edge cases and JSON parsing
- Run with `cargo test`

---

## Prerequisites

- **Rust 1.99.0+** ([install](https://rustup.rs/))
- **AWS credentials** configured (profile, env vars, or IAM role)
- **For SSM integration**: [`session-manager-plugin`](https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html) (optional)

---

## Installation

### Build from Source

```bash
git clone https://github.com/bartleboeuf/cloudshell-rs.git
cd cloudshell-rs
cargo build --release
```

The binary is at `target/release/cloudshell-rs`.

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
cloudshell-rs = { path = "../cloudshell-rs" }
tokio = { version = "1.40", features = ["full"] }
```

---

## Quick Start

### CLI Usage

**Help & Documentation:**
```bash
# Show available commands
./target/release/cloudshell-rs --help

# Show detailed help for a command
./target/release/cloudshell-rs create --help
./target/release/cloudshell-rs connect --help
./target/release/cloudshell-rs heartbeat --help

# Each command includes usage examples, prerequisites, and security notes
```

**Environment Management:**
```bash
# List environments
./target/release/cloudshell-rs describe

# Create public environment
./target/release/cloudshell-rs create
./target/release/cloudshell-rs create --name my-env

# Create VPC environment
./target/release/cloudshell-rs create --name my-vpc-env \
  --vpc-id vpc-0123456789abcdef0 \
  --subnet-ids subnet-1,subnet-2 \
  --security-group-ids sg-1,sg-2

# Check status
./target/release/cloudshell-rs status abc123-def4-5678-ghij

# Start environment
./target/release/cloudshell-rs start abc123-def4-5678-ghij

# Stop environment
./target/release/cloudshell-rs stop abc123-def4-5678-ghij

# Delete environment
./target/release/cloudshell-rs delete abc123-def4-5678-ghij
```

**Sessions & Connection:**
```bash
# Create session (returns stream URL and token)
./target/release/cloudshell-rs session abc123-def4-5678-ghij --tab-id $(uuidgen)

# Connect to environment via SSM Session Manager (interactive shell)
./target/release/cloudshell-rs connect

# Connect to specific environment
./target/release/cloudshell-rs connect abc123-def4-5678-ghij

# Inject AWS credentials into the remote shell (see "Credentials in CloudShell Sessions")
./target/release/cloudshell-rs connect --inject-credentials

# Increase environment wait timeout
./target/release/cloudshell-rs connect --timeout-secs 300
```

**Maintenance & File Transfer:**
```bash
# Send heartbeat (keeps environment alive)
./target/release/cloudshell-rs heartbeat abc123-def4-5678-ghij

# Get file upload presigned URLs
./target/release/cloudshell-rs upload abc123-def4-5678-ghij

# Get file download presigned URLs
./target/release/cloudshell-rs download abc123-def4-5678-ghij
```

**With AWS profile and region:**
```bash
./target/release/cloudshell-rs --profile myprofile --region us-east-1 describe
./target/release/cloudshell-rs --profile myprofile --region eu-west-1 status abc-123
./target/release/cloudshell-rs --profile myprofile --region us-east-1 connect
```

### Library Usage

```rust
use cloudshell_rs::CloudShellClient;
use cloudshell_rs::progress::SilentProgress;
use cloudshell_rs::workflow::{get_or_create_env, wait_for_running};
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> cloudshell_rs::CloudResult<()> {
    let client = CloudShellClient::new(
        "us-east-1".to_string(),
        Some("myprofile".to_string()),
    )
    .await?;

    // Find an existing environment, or create a public one (progress goes to `SilentProgress`)
    let env_id = get_or_create_env(&client, &SilentProgress).await?;

    // Poll until RUNNING (starts a SUSPENDED environment), with a timeout
    wait_for_running(&client, &env_id, Duration::from_secs(120), &SilentProgress).await?;

    // Create a session (TabId is validated as UUID v4)
    let tab_id = Uuid::new_v4().to_string();
    let session = client
        .create_session(&env_id, "TMUX", &tab_id, Some(true))
        .await?;
    let session_id = session.session_id().unwrap_or_default();

    // Get file URLs
    let upload = client.get_file_upload_urls(&env_id).await?;
    println!(
        "Upload URL: {}",
        upload.file_upload_presigned_url().unwrap_or_default()
    );

    // Send heartbeat, then clean up the session
    client.send_heart_beat(&env_id).await?;
    client.delete_session(&env_id, &session_id).await?;

    Ok(())
}
```

`CloudShellError` implements `std::error::Error`, so `?` also works in functions returning `Box<dyn Error>`.

To create a VPC environment instead, build a `VpcConfig` and pass it to `create_environment`:

```rust
let vpc = VpcConfig::new(
    "vpc-0123456789abcdef0".to_string(),
    vec!["subnet-1".to_string()],
    vec!["sg-1".to_string()],
);
let created = client.create_environment(Some("my-vpc-env"), Some(vpc)).await?;
```

---

## Examples

Two example programs are included:

### 1. Full Lifecycle

Complete environment workflow with file operations:

```bash
cargo run --example full_lifecycle -- --profile myprofile --region us-east-1
```

**What it does:**
- Creates/finds environment (via the shared `workflow` helpers)
- Waits for RUNNING state
- Creates session
- Gets upload/download presigned URLs
- Sends heartbeat
- Cleans up session
- Keeps environment for reuse

**See:** `examples/full_lifecycle.rs`

### 2. SSM Session Manager Integration

Interactive shell access with AWS Systems Manager:

```bash
# Interactive shell
cargo run --example connect_via_ssm -- --profile myprofile --region us-east-1

# Inject AWS credentials (the flag takes no value)
cargo run --example connect_via_ssm -- --profile myprofile --inject-credentials
```

**What it does:**
- Verifies `session-manager-plugin` is installed
- Creates environment + session
- Spawns `session-manager-plugin` subprocess
- Provides interactive shell or securely injects AWS credentials

**Modes:**
- **Interactive** (default): Full shell access via `session-manager-plugin`. Type `exit` to disconnect
- **Credential injection** (`--inject-credentials`): loads the current credentials into the remote bash shell through a temporary file (see [Credentials in CloudShell Sessions](#credentials-in-cloudshell-sessions)). The shell is used once injection is confirmed

**Prerequisites:** `session-manager-plugin` installed ([macOS](https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html) | [Linux](https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html))

**See:** `examples/connect_via_ssm.rs`

---

## Development

### Build & Test

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Lint check (all targets, warnings as errors)
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt
```

### Project Structure

```
src/
├── lib.rs          # CloudShellClient, CloudShellError, CloudResult, VpcConfig, response types
│                   #   - 12 API operations
│                   #   - Retry policy and unit tests
├── workflow.rs     # get_or_create_env, wait_for_running (shared by CLI and examples)
├── ssm.rs          # connect_via_ssm, check_session_manager_plugin, credential loading
│                   #   - Unit tests for shell quoting
├── progress.rs     # Progress trait, StdoutProgress, SilentProgress
└── main.rs         # CLI subcommands (thin wrapper over the library)
                    #   - Comprehensive help text
                    #   - VPC environment support

examples/
├── full_lifecycle.rs        # Complete workflow demo
└── connect_via_ssm.rs       # SSM integration demo

tests/
└── integration_tests.rs    # Integration tests (response types, VPC and UUID validation, edge cases)
```

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| `aws-config` | AWS credential resolution |
| `aws-sigv4` | SigV4 request signing |
| `reqwest` | Async HTTP client |
| `aws-smithy-runtime` | Provides the default HTTPS client used by `aws-config` (required at runtime) |
| `tokio` | Async runtime |
| `clap` | CLI argument parsing |
| `serde_json` | JSON serialization |
| `uuid` | UUID v4 generation |

---

## API Reference

### CloudShellClient Methods

All methods are async and return `CloudResult<T>`, which is `Result<T, CloudShellError>`. `CloudShellError` implements `std::error::Error`, so `?` also works in functions returning `Box<dyn Error>`:

```rust
// Environment management
pub async fn describe_environments(&self) -> CloudResult<DescribeEnvironmentsResponse>
pub async fn create_environment(&self, name: Option<&str>, vpc: Option<VpcConfig>)
    -> CloudResult<CreateEnvironmentResponse>
pub async fn get_environment_status(&self, env_id: &str) -> CloudResult<EnvironmentStatusResponse>
pub async fn start_environment(&self, env_id: &str) -> CloudResult<()>
pub async fn stop_environment(&self, env_id: &str) -> CloudResult<StopEnvironmentResponse>
pub async fn delete_environment(&self, env_id: &str) -> CloudResult<()>

// Session management
pub async fn create_session(&self, env_id: &str, session_type: &str, tab_id: &str,
    q_cli_disabled: Option<bool>) -> CloudResult<CreateSessionResponse>
pub async fn delete_session(&self, env_id: &str, session_id: &str) -> CloudResult<()>

// Heartbeat
pub async fn send_heart_beat(&self, env_id: &str) -> CloudResult<()>

// File transfer
pub async fn get_file_upload_urls(&self, env_id: &str) -> CloudResult<FileTransferResponse>
pub async fn get_file_download_urls(&self, env_id: &str) -> CloudResult<FileTransferResponse>

// Credentials (console sessions only)
pub async fn put_credentials(&self, env_id: &str) -> CloudResult<()>
```

### Workflow, SSM and Progress Helpers

These shared helpers are used by the CLI and the examples. They are public so you can reuse them.

```rust
// cloudshell_rs::workflow
pub async fn get_or_create_env(client: &CloudShellClient, progress: &dyn Progress) -> CloudResult<String>
pub async fn wait_for_running(client: &CloudShellClient, env_id: &str,
    timeout: Duration, progress: &dyn Progress) -> CloudResult<()>

// cloudshell_rs::ssm
pub fn check_session_manager_plugin() -> bool
pub async fn connect_via_ssm(client: &CloudShellClient, env_id: &str, session_id: &str,
    payload: &str, region: &str, inject_credentials: bool,
    progress: &dyn Progress) -> CloudResult<()>

// cloudshell_rs::progress
pub trait Progress { fn info(&self, message: &str); }
pub struct StdoutProgress;  // prints messages (used by the CLI and examples)
pub struct SilentProgress;  // discards messages
```

The library does not print on its own. Pass `&SilentProgress` to run quietly, or implement `Progress` to route messages to your own logger. Output from `session-manager-plugin` itself is always shown on the terminal.

### VpcConfig

For creating VPC environments with input validation:

```rust
// Construct; create_environment() validates it before calling the API
let vpc = VpcConfig::new(
    "vpc-0123456789abcdef0".to_string(),
    vec!["subnet-1".to_string(), "subnet-2".to_string()],
    vec!["sg-1".to_string()],
);

// Or validate explicitly (returns Result)
let vpc = VpcConfig {
    vpc_id: "vpc-0123456789abcdef0".to_string(),
    subnet_ids: vec!["subnet-1".to_string()],
    security_group_ids: vec!["sg-1".to_string(), "sg-2".to_string()],
};

match vpc.validate() {
    Ok(_) => {
        // VPC config is valid
        let response = client.create_environment(Some("my-vpc-env"), Some(vpc)).await?;
    }
    Err(e) => eprintln!("Invalid VPC config: {}", e),
}
```

**Validation rules:**
- Min 1 subnet required
- Max 5 security groups allowed
- `VpcConfig::new()` does not validate. Invalid input is reported by `validate()` (returns `Result<(), String>`) and by `create_environment()` as `CloudShellError::Validation`
- VPC ID, subnet IDs and security group IDs must not be blank

### Response Types

Each response type has helper methods:

```rust
// DescribeEnvironmentsResponse
response.environments() -> Vec<serde_json::Value>
response.pretty_print() -> Result<String, ...>

// CreateEnvironmentResponse
response.environment_id() -> Option<String>
response.status() -> Option<String>

// EnvironmentStatusResponse
response.status() -> Option<String>
response.environment_id() -> Option<String>
response.status_reason() -> Option<String>

// CreateSessionResponse
response.session_id() -> Option<String>
response.token_value() -> Option<String>
response.stream_url() -> Option<String>

// FileTransferResponse
response.file_upload_presigned_url() -> Option<String>
response.file_upload_presigned_fields() -> Option<serde_json::Value>
response.file_download_presigned_url() -> Option<String>
response.file_download_presigned_key() -> Option<String>
response.file_download_presigned_key_hash() -> Option<String>
```

---

## IAM Permissions

Ensure your AWS principal has these CloudShell permissions:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "cloudshell:DescribeEnvironments",
        "cloudshell:GetEnvironmentStatus",
        "cloudshell:CreateEnvironment",
        "cloudshell:DeleteEnvironment",
        "cloudshell:StartEnvironment",
        "cloudshell:StopEnvironment",
        "cloudshell:CreateSession",
        "cloudshell:DeleteSession",
        "cloudshell:SendHeartBeat",
        "cloudshell:GetFileUploadUrls",
        "cloudshell:GetFileDownloadUrls"
      ],
      "Resource": "*"
    }
  ]
}
```

For VPC environments, also add EC2 permissions: `ec2:CreateNetworkInterface`, `ec2:CreateTags`, etc.

---

## Input Validation

The library includes built-in validation for API parameters:

### VpcConfig Validation

```rust
// Enforced automatically
- Min 1 subnet required
- Max 5 security groups allowed
```

### TabId Validation

```rust
// Session creation validates UUID v4 format
let response = client.create_session(
    &env_id,
    "TMUX",
    &Uuid::new_v4().to_string(),  // Must be valid UUID v4
    Some(true)
).await?;

// Invalid UUID returns error before API call
let invalid = client.create_session(
    &env_id,
    "TMUX", 
    "not-a-uuid",  // ❌ Error: Invalid TabId
    None
).await; // Err: "Invalid TabId: 'not-a-uuid' is not a valid UUID"
```

---

## How It Works

CloudShell uses the `rest-json` protocol — operations are routed by **URI path** (e.g., `/describeEnvironments`), not HTTP headers.

**Request flow:**
1. Build JSON request body
2. Sign with SigV4 (AWS credentials + service + region)
3. POST to `https://cloudshell.<region>.amazonaws.com/<operation>`
4. Parse JSON response
5. Return typed response wrapper

**Timeouts and retries:** every request has a 30-second timeout. Read-only calls (`DescribeEnvironments`, `GetEnvironmentStatus`) retry up to 3 attempts on connection or timeout errors and on HTTP 429/5xx responses. Mutating calls (create, start, stop, session, delete, heartbeat) are never retried, to avoid duplicate side effects.

**Why manual signing?**

Standard AWS SDKs may not support the CloudShell service definition, so this project uses the `aws-sigv4` crate for direct request signing. This gives full control over the request format.

---

## Limits

- **Max 5 security groups** per VPC environment (enforced by the client before the API call)
- Other quotas (VPC environment count, storage) are set by AWS and may change; check the CloudShell documentation
- Environments **auto-sleep** after inactivity (use heartbeats to prevent)
- Environments can be **reclaimed by AWS** without notice

---

## Credentials in CloudShell Sessions

`connect --inject-credentials` loads the credentials of the active profile (or `--profile`) into the remote bash shell, so AWS CLI commands work without extra setup.

```bash
./target/release/cloudshell-rs connect --inject-credentials
```

**What happens:**
1. Echo and shell history are turned off in the remote shell.
2. `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN` (if present) and `AWS_DEFAULT_REGION` are written to a mode-`600` file in `/dev/shm` (RAM-backed), with values shell-quoted.
3. The file is sourced and deleted immediately. History and echo are restored.
4. The CLI waits for a marker confirming the setup ran, instead of fixed delays.
5. A `trap` unsets the variables when the shell exits.

**Limitations:**
- The secrets are still typed into the SSM input stream. If SSM session logging is enabled for the account, the commands may be recorded.
- Requires bash in the environment.
- If the active profile uses long-term access keys, those are injected. Prefer short-lived credentials (for example, from `sts:AssumeRole`).
- Credentials are held as plain strings in CLI memory and are not zeroized.
- If injection fails, the CLI stops the plugin and deletes the session before returning the error.

---

## Related Resources

- **Python Reference**: [cloudshell-boto3](https://github.com/guyon-it-consulting/cloudshell-boto3) — upstream project with Python examples
- **AWS CloudShell Docs**: https://docs.aws.amazon.com/cloudshell/latest/userguide/
- **AWS SigV4 Signing**: https://docs.aws.amazon.com/general/latest/gr/signature-version-4.html
- **Smithy Language**: https://smithy.io/ — service model format
- **CloudShell Deep Dive**: https://awsteele.com/blog/2024/01/11/deep-dive-into-aws-cloudshell.html

---

## License

MIT License. See `LICENSE` for details.

---

## Contributing

Found a bug? Have a feature request? Open an issue on [GitHub](https://github.com/bartleboeuf/cloudshell-rs/issues).

---

## Author

Created and maintained by **[Bart Leboeuf](https://github.com/bartleboeuf)**.

---

## Disclaimer

This project reverse-engineers an undocumented AWS API. AWS can change or deprecate it at any time. Use at your own risk.

For mission-critical workloads, stick with officially documented AWS services.
