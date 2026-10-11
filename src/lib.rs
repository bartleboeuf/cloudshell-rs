use aws_config::BehaviorVersion;
use aws_credential_types::provider::ProvideCredentials;
use aws_sigv4::http_request::{
    SignableBody, SignableRequest, SigningParams, SigningSettings, sign,
};
use std::time::{Duration, SystemTime};

pub mod heartbeat;
pub mod progress;
pub mod ssm;
pub mod workflow;

#[derive(Debug, Clone)]
pub enum CloudShellError {
    Config(String),
    Credential(String),
    Validation(String),
    Json(String),
    Request(String),
    Network(String),
    Aws {
        status: String,
        message: String,
    },
    InvalidUuid(String),
    Io(String),
    Process(String),
    Response(String),
    Timeout(String),
    /// A service quota or account limit was exceeded (e.g. 200 h/month usage quota)
    Quota(String),
    /// The API throttled the request (retried automatically; surfaced if retries are exhausted)
    Throttled(String),
    /// Transient server-side failure (HTTP 5xx)
    Transient(String),
    /// Credentials could not be decrypted (e.g. `put_credentials` without a console token)
    TokenDecryption(String),
}

impl std::fmt::Display for CloudShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(message)
            | Self::Credential(message)
            | Self::Validation(message)
            | Self::Json(message)
            | Self::Request(message)
            | Self::Network(message)
            | Self::InvalidUuid(message)
            | Self::Io(message)
            | Self::Process(message)
            | Self::Response(message)
            | Self::Timeout(message)
            | Self::Quota(message)
            | Self::Throttled(message)
            | Self::Transient(message)
            | Self::TokenDecryption(message) => write!(f, "{message}"),
            Self::Aws { status, message } => write!(f, "AWS request failed ({status}): {message}"),
        }
    }
}

impl std::error::Error for CloudShellError {}

impl From<serde_json::Error> for CloudShellError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err.to_string())
    }
}

impl From<reqwest::Error> for CloudShellError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network(err.to_string())
    }
}

impl From<std::io::Error> for CloudShellError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

pub type CloudResult<T> = std::result::Result<T, CloudShellError>;

/// CloudShell API client for AWS CloudShell operations
pub struct CloudShellClient {
    region: String,
    credentials: aws_credential_types::Credentials,
    http_client: reqwest::Client,
}

impl CloudShellClient {
    /// Create a new CloudShell client with credentials from AWS configuration
    pub async fn new(region: String, profile: Option<String>) -> CloudResult<Self> {
        // Load AWS configuration with optional profile
        let mut config_builder = aws_config::defaults(BehaviorVersion::latest())
            .sleep_impl(aws_smithy_async::rt::sleep::TokioSleep::new());
        if let Some(profile) = profile {
            config_builder = config_builder.profile_name(&profile);
        }
        let config = config_builder.load().await;

        // Get credentials
        let credentials_provider = config.credentials_provider().ok_or_else(|| {
            CloudShellError::Config("No credentials provider available".to_string())
        })?;
        let credentials = credentials_provider
            .provide_credentials()
            .await
            .map_err(|err| CloudShellError::Credential(err.to_string()))?;

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|err| CloudShellError::Config(err.to_string()))?;

        Ok(CloudShellClient {
            region,
            credentials,
            http_client,
        })
    }

    /// Get the current AWS credentials (for manual credential injection)
    pub fn get_credentials(&self) -> &aws_credential_types::Credentials {
        &self.credentials
    }

    /// Describe all CloudShell environments
    pub async fn describe_environments(&self) -> CloudResult<DescribeEnvironmentsResponse> {
        let json = self
            .call_api_json("describeEnvironments", &serde_json::json!({}))
            .await?;
        Ok(DescribeEnvironmentsResponse { raw_response: json })
    }

    /// Create a new CloudShell environment
    ///
    /// # Arguments
    /// * `environment_name` - Optional name (required for VPC environments)
    /// * `vpc_config` - Optional VPC configuration (None = public environment)
    ///
    /// # Example: Public environment
    /// ```ignore
    /// let response = client.create_environment(None, None).await?;
    /// ```
    ///
    /// # Example: VPC environment
    /// ```ignore
    /// let vpc = VpcConfig::new(
    ///     "vpc-123".to_string(),
    ///     vec!["subnet-1".to_string()],
    ///     vec!["sg-1".to_string()],
    /// );
    /// let response = client.create_environment(Some("my-env"), Some(vpc)).await?;
    /// ```
    pub async fn create_environment(
        &self,
        environment_name: Option<&str>,
        vpc_config: Option<VpcConfig>,
    ) -> CloudResult<CreateEnvironmentResponse> {
        let mut body = serde_json::json!({});
        if let Some(name) = environment_name {
            body["EnvironmentName"] = serde_json::json!(name);
        }
        if let Some(vpc) = &vpc_config {
            vpc.validate().map_err(|err| {
                CloudShellError::Validation(format!("Invalid VPC configuration: {err}"))
            })?;
            body["VpcConfig"] = serde_json::json!({
                "VpcId": vpc.vpc_id,
                "SubnetIds": vpc.subnet_ids,
                "SecurityGroupIds": vpc.security_group_ids,
            });
        }

        if vpc_config.is_some() {
            self.ensure_vpc_quota_available().await?;
        }

        let json = self.call_api_json("createEnvironment", &body).await?;

        Ok(CreateEnvironmentResponse { raw_response: json })
    }

    /// Start a CloudShell environment
    pub async fn start_environment(&self, environment_id: &str) -> CloudResult<()> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        self.call_api("startEnvironment", &body).await?;
        Ok(())
    }

    /// Stop a CloudShell environment
    pub async fn stop_environment(
        &self,
        environment_id: &str,
    ) -> CloudResult<StopEnvironmentResponse> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        let json = self.call_api_json("stopEnvironment", &body).await?;

        Ok(StopEnvironmentResponse { raw_response: json })
    }

    /// Get the status of a CloudShell environment
    pub async fn get_environment_status(
        &self,
        environment_id: &str,
    ) -> CloudResult<EnvironmentStatusResponse> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        let json = self.call_api_json("getEnvironmentStatus", &body).await?;

        Ok(EnvironmentStatusResponse { raw_response: json })
    }

    /// Create a session to a CloudShell environment
    ///
    /// # Arguments
    /// * `environment_id` - The environment ID to connect to
    /// * `session_type` - Session type (usually "TMUX")
    /// * `tab_id` - A valid UUID v4 string (use `uuid::Uuid::new_v4().to_string()`)
    /// * `q_cli_disabled` - Optional flag to disable Amazon Q CLI integration
    ///
    /// # Errors
    /// Returns an error if `tab_id` is not a valid UUID v4
    ///
    /// # Example
    /// ```ignore
    /// use uuid::Uuid;
    /// let tab_id = Uuid::new_v4().to_string();
    /// let session = client.create_session(&env_id, "TMUX", &tab_id, Some(true)).await?;
    /// ```
    pub async fn create_session(
        &self,
        environment_id: &str,
        session_type: &str,
        tab_id: &str,
        q_cli_disabled: Option<bool>,
    ) -> CloudResult<CreateSessionResponse> {
        // Validate tab_id is a valid UUID
        if uuid::Uuid::parse_str(tab_id).is_err() {
            return Err(CloudShellError::InvalidUuid(format!(
                "Invalid TabId: '{}' is not a valid UUID",
                tab_id
            )));
        }

        let mut body = serde_json::json!({
            "EnvironmentId": environment_id,
            "SessionType": session_type,
            "TabId": tab_id,
        });
        if let Some(disabled) = q_cli_disabled {
            body["QCliDisabled"] = serde_json::json!(disabled);
        }

        let json = self.call_api_json("createSession", &body).await?;

        Ok(CreateSessionResponse { raw_response: json })
    }

    /// Send a heartbeat to keep a CloudShell environment alive
    ///
    /// For a periodic heartbeat, see [`crate::heartbeat::spawn_heartbeat`].
    pub async fn send_heart_beat(&self, environment_id: &str) -> CloudResult<()> {
        require_non_empty("EnvironmentId", environment_id)?;
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        self.call_api("sendHeartBeat", &body).await?;
        Ok(())
    }

    /// Delete a CloudShell session
    pub async fn delete_session(&self, environment_id: &str, session_id: &str) -> CloudResult<()> {
        require_non_empty("EnvironmentId", environment_id)?;
        require_non_empty("SessionId", session_id)?;
        let body = serde_json::json!({
            "EnvironmentId": environment_id,
            "SessionId": session_id,
        });
        self.call_api("deleteSession", &body).await?;
        Ok(())
    }

    /// Delete a CloudShell environment
    ///
    /// Pre-validates the environment state: the API only accepts deletes for
    /// environments in `RUNNING` or `SUSPENDED` state. This costs one extra
    /// `GetEnvironmentStatus` call.
    pub async fn delete_environment(&self, environment_id: &str) -> CloudResult<()> {
        require_non_empty("EnvironmentId", environment_id)?;

        let status = self.get_environment_status(environment_id).await?.status();
        match status.as_deref() {
            Some("RUNNING") | Some("SUSPENDED") => {}
            other => {
                return Err(CloudShellError::Validation(format!(
                    "Cannot delete environment {environment_id} in state {}: it must be RUNNING or SUSPENDED",
                    other.unwrap_or("UNKNOWN")
                )));
            }
        }

        let body = serde_json::json!({ "EnvironmentId": environment_id });
        self.call_api("deleteEnvironment", &body).await?;
        Ok(())
    }

    /// Get file upload URLs
    pub async fn get_file_upload_urls(
        &self,
        environment_id: &str,
    ) -> CloudResult<FileTransferResponse> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        let json = self.call_api_json("getFileUploadUrls", &body).await?;

        Ok(FileTransferResponse { raw_response: json })
    }

    /// Get file download URLs
    pub async fn get_file_download_urls(
        &self,
        environment_id: &str,
    ) -> CloudResult<FileTransferResponse> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        let json = self.call_api_json("getFileDownloadUrls", &body).await?;

        Ok(FileTransferResponse { raw_response: json })
    }

    /// Put credentials into a CloudShell environment
    ///
    /// **Console-only:** the API requires a console session token. Programmatic callers
    /// get [`CloudShellError::TokenDecryption`]. To inject credentials, export the `AWS_*`
    /// variables over an SSM session instead (see `connect --inject-credentials`).
    pub async fn put_credentials(&self, environment_id: &str) -> CloudResult<()> {
        require_non_empty("EnvironmentId", environment_id)?;
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        self.call_api("putCredentials", &body).await?;
        Ok(())
    }

    async fn call_api_json(
        &self,
        operation: &str,
        body: &serde_json::Value,
    ) -> CloudResult<serde_json::Value> {
        let response = self.call_api(operation, body).await?;
        Ok(serde_json::from_str(&response)?)
    }

    /// Pre-validate the VPC environment quota (max [`MAX_VPC_ENVIRONMENTS`] per principal).
    ///
    /// Best-effort: counts environments whose describe output includes a `VpcConfig` key.
    async fn ensure_vpc_quota_available(&self) -> CloudResult<()> {
        let existing = self
            .describe_environments()
            .await?
            .environments()
            .iter()
            .filter(|env| env.get("VpcConfig").is_some())
            .count();

        if existing >= MAX_VPC_ENVIRONMENTS {
            return Err(CloudShellError::Quota(format!(
                "VPC environment limit reached: {existing}/{MAX_VPC_ENVIRONMENTS} already exist for this principal"
            )));
        }
        Ok(())
    }

    /// Call a CloudShell API operation, retrying transient failures with exponential backoff.
    ///
    /// Retry policy (see [`should_retry`]):
    /// - Throttling is retried for every operation (the request was rejected, nothing ran).
    /// - 5xx and network errors are retried only for operations in [`is_retry_safe_operation`].
    /// - Quota errors are never retried.
    async fn call_api(&self, operation: &str, body: &serde_json::Value) -> CloudResult<String> {
        let body_bytes = serde_json::to_vec(body)?;
        let retry_safe = is_retry_safe_operation(operation);
        let mut attempt: u32 = 1;

        loop {
            match self.send_signed(operation, &body_bytes).await {
                Ok(response_body) => return Ok(response_body),
                Err(err) if attempt < RETRY_MAX_ATTEMPTS && should_retry(&err, retry_safe) => {
                    tokio::time::sleep(backoff_delay(attempt)).await;
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Sign (with a fresh timestamp) and send a single HTTP request.
    async fn send_signed(&self, operation: &str, body_bytes: &[u8]) -> CloudResult<String> {
        let service = "cloudshell";
        let host = format!("{}.{}.amazonaws.com", service, self.region);
        let url = format!("https://{}/{}", host, operation);

        // Create signable request for SigV4 signing
        let headers_for_signing = [
            ("Content-Type", "application/x-amz-json-1.1"),
            ("host", host.as_str()),
        ];

        let signable_request = SignableRequest::new(
            "POST",
            &url,
            headers_for_signing.iter().copied(),
            SignableBody::Bytes(body_bytes),
        )
        .map_err(|err| CloudShellError::Request(err.to_string()))?;

        // Convert credentials to Identity for signing
        let identity = self.credentials.clone().into();

        // Sign the request with SigV4
        let signing_params = SigningParams::V4(
            aws_sigv4::sign::v4::SigningParams::builder()
                .identity(&identity)
                .region(&self.region)
                .name(service)
                .time(SystemTime::now())
                .settings(SigningSettings::default())
                .build()
                .map_err(|err| CloudShellError::Request(err.to_string()))?,
        );
        let signing_result = sign(signable_request, &signing_params)
            .map_err(|err| CloudShellError::Request(err.to_string()))?;
        let signing_instructions = signing_result.output();

        // Build final request with all headers
        let mut final_headers = reqwest::header::HeaderMap::new();
        final_headers.insert(
            "Content-Type"
                .parse::<reqwest::header::HeaderName>()
                .map_err(|err| CloudShellError::Request(err.to_string()))?,
            "application/x-amz-json-1.1"
                .parse::<reqwest::header::HeaderValue>()
                .map_err(|err| CloudShellError::Request(err.to_string()))?,
        );

        // Add SigV4 signed headers
        for (name, value) in signing_instructions.headers() {
            final_headers.insert(
                name.parse::<reqwest::header::HeaderName>()
                    .map_err(|err| CloudShellError::Request(err.to_string()))?,
                value
                    .parse::<reqwest::header::HeaderValue>()
                    .map_err(|err| CloudShellError::Request(err.to_string()))?,
            );
        }

        let response = self
            .http_client
            .post(&url)
            .headers(final_headers)
            .body(body_bytes.to_vec())
            .send()
            .await?;
        let status = response.status();
        let response_body = response.text().await?;

        if status.is_success() {
            Ok(response_body)
        } else {
            Err(parse_api_error(status, &response_body))
        }
    }
}

/// Maximum number of VPC environments per IAM principal.
pub const MAX_VPC_ENVIRONMENTS: usize = 2;

/// Maximum number of security groups per VPC environment.
pub const MAX_SECURITY_GROUPS: usize = 5;

/// Total attempts (initial + retries) for a retryable API call.
const RETRY_MAX_ATTEMPTS: u32 = 5;
const RETRY_BASE_DELAY_MS: u64 = 250;
const RETRY_MAX_DELAY_MS: u64 = 8_000;

fn require_non_empty(name: &str, value: &str) -> CloudResult<()> {
    if value.trim().is_empty() {
        Err(CloudShellError::Validation(format!(
            "{name} cannot be empty"
        )))
    } else {
        Ok(())
    }
}

/// Operations with no side effects or where repeating them is harmless.
/// Creates and deletes are excluded: a retry after a server-side success would
/// duplicate the resource or report a confusing not-found error.
fn is_retry_safe_operation(operation: &str) -> bool {
    matches!(
        operation,
        "describeEnvironments"
            | "getEnvironmentStatus"
            | "sendHeartBeat"
            | "startEnvironment"
            | "stopEnvironment"
            | "getFileUploadUrls"
            | "getFileDownloadUrls"
    )
}

fn should_retry(err: &CloudShellError, retry_safe: bool) -> bool {
    match err {
        CloudShellError::Throttled(_) => true,
        CloudShellError::Transient(_) | CloudShellError::Network(_) => retry_safe,
        _ => false,
    }
}

/// Exponential backoff with jitter: the delay for `attempt` (1-based) is in
/// `[cap/2, cap]` where `cap = min(base * 2^(attempt-1), max)`.
fn backoff_delay(attempt: u32) -> Duration {
    let exp = RETRY_BASE_DELAY_MS.saturating_mul(1u64 << attempt.saturating_sub(1).min(10));
    let cap = exp.min(RETRY_MAX_DELAY_MS);
    let jitter = uuid::Uuid::new_v4().as_u128() as u64 % (cap / 2 + 1);
    Duration::from_millis(cap / 2 + jitter)
}

/// Map an HTTP error response to a typed error using the AWS JSON error `__type` code.
fn parse_api_error(status: reqwest::StatusCode, body: &str) -> CloudShellError {
    let json: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let code = json
        .as_ref()
        .and_then(|j| j.get("__type").or_else(|| j.get("code")))
        .and_then(|v| v.as_str())
        .map(|t| t.rsplit(['#', ':']).next().unwrap_or(t).to_string())
        .unwrap_or_default();
    let message = json
        .as_ref()
        .and_then(|j| j.get("message").or_else(|| j.get("Message")))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| body.trim().to_string());
    let detail = if code.is_empty() {
        message
    } else {
        format!("{code}: {message}")
    };

    match code.as_str() {
        "ThrottlingException" | "TooManyRequestsException" | "RequestLimitExceeded" => {
            CloudShellError::Throttled(detail)
        }
        "TokenDecryptionException" => CloudShellError::TokenDecryption(format!(
            "{detail} (put_credentials requires a console session token; export AWS_* variables over SSM instead)"
        )),
        c if c.contains("Quota") || c.contains("LimitExceeded") => CloudShellError::Quota(detail),
        _ if status == reqwest::StatusCode::TOO_MANY_REQUESTS => CloudShellError::Throttled(detail),
        _ if status.is_server_error() => CloudShellError::Transient(detail),
        _ => CloudShellError::Aws {
            status: status.to_string(),
            message: detail,
        },
    }
}

/// VPC configuration for environments
///
/// **Limits:**
/// - Max 5 security groups per environment
/// - See: https://docs.aws.amazon.com/cloudshell/latest/userguide/aws-cloudshell-vpc-permissions-1.html
#[derive(Debug, Clone)]
pub struct VpcConfig {
    pub vpc_id: String,
    pub subnet_ids: Vec<String>,
    pub security_group_ids: Vec<String>,
}

impl VpcConfig {
    /// Create a new VPC configuration
    ///
    /// # Arguments
    /// * `vpc_id` - The VPC ID (e.g., "vpc-0123456789abcdef0")
    /// * `subnet_ids` - List of subnet IDs (at least 1 required)
    /// * `security_group_ids` - List of security group IDs (max 5)
    ///
    /// # Example
    /// ```
    /// use cloudshell_rs::VpcConfig;
    /// let vpc = VpcConfig::new(
    ///     "vpc-123".to_string(),
    ///     vec!["subnet-1".to_string()],
    ///     vec!["sg-1".to_string()],
    /// );
    /// ```
    pub fn new(vpc_id: String, subnet_ids: Vec<String>, security_group_ids: Vec<String>) -> Self {
        VpcConfig {
            vpc_id,
            subnet_ids,
            security_group_ids,
        }
    }

    /// Validate the VPC configuration
    ///
    /// Returns an error if the configuration is invalid (e.g., too many security groups)
    ///
    /// # Example
    /// ```
    /// use cloudshell_rs::VpcConfig;
    /// let vpc = VpcConfig {
    ///     vpc_id: "vpc-123".to_string(),
    ///     subnet_ids: vec!["subnet-1".to_string()],
    ///     security_group_ids: vec!["sg-1".to_string(); 6],  // Too many
    /// };
    /// assert!(vpc.validate().is_err());
    /// ```
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.vpc_id.trim().is_empty() {
            return Err("VPC ID cannot be empty".to_string());
        }
        if self.security_group_ids.len() > 5 {
            return Err(format!(
                "Max 5 security groups allowed, got {}",
                self.security_group_ids.len()
            ));
        }
        if self.subnet_ids.is_empty() {
            return Err("At least 1 subnet is required".to_string());
        }
        if self.subnet_ids.iter().any(|id| id.trim().is_empty()) {
            return Err("Subnet IDs cannot be empty".to_string());
        }
        if self
            .security_group_ids
            .iter()
            .any(|id| id.trim().is_empty())
        {
            return Err("Security group IDs cannot be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod retry_tests {
    use super::*;

    #[test]
    fn retry_safe_operations_exclude_creates_and_deletes() {
        assert!(is_retry_safe_operation("describeEnvironments"));
        assert!(is_retry_safe_operation("sendHeartBeat"));
        assert!(!is_retry_safe_operation("createEnvironment"));
        assert!(!is_retry_safe_operation("createSession"));
        assert!(!is_retry_safe_operation("deleteEnvironment"));
    }

    #[test]
    fn throttling_is_retried_for_all_operations() {
        let err = CloudShellError::Throttled("x".into());
        assert!(should_retry(&err, false));
        assert!(should_retry(&err, true));
    }

    #[test]
    fn transient_errors_only_retried_when_safe() {
        let err = CloudShellError::Transient("x".into());
        assert!(should_retry(&err, true));
        assert!(!should_retry(&err, false));
    }

    #[test]
    fn quota_and_validation_errors_never_retried() {
        assert!(!should_retry(&CloudShellError::Quota("x".into()), true));
        assert!(!should_retry(
            &CloudShellError::Validation("x".into()),
            true
        ));
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        for attempt in 1..=10 {
            let d = backoff_delay(attempt).as_millis() as u64;
            assert!(d <= RETRY_MAX_DELAY_MS, "attempt {attempt}: {d}ms");
        }
        assert!(backoff_delay(1).as_millis() as u64 <= RETRY_BASE_DELAY_MS);
    }

    #[test]
    fn parses_throttling_and_quota_codes() {
        let body =
            r#"{"__type":"com.amazonaws.cloudshell#ThrottlingException","message":"slow down"}"#;
        assert!(matches!(
            parse_api_error(reqwest::StatusCode::BAD_REQUEST, body),
            CloudShellError::Throttled(_)
        ));

        let body = r#"{"__type":"ServiceQuotaExceededException","message":"200 hours"}"#;
        assert!(matches!(
            parse_api_error(reqwest::StatusCode::BAD_REQUEST, body),
            CloudShellError::Quota(_)
        ));
    }

    #[test]
    fn maps_http_status_when_no_code() {
        assert!(matches!(
            parse_api_error(reqwest::StatusCode::TOO_MANY_REQUESTS, ""),
            CloudShellError::Throttled(_)
        ));
        assert!(matches!(
            parse_api_error(reqwest::StatusCode::SERVICE_UNAVAILABLE, ""),
            CloudShellError::Transient(_)
        ));
        assert!(matches!(
            parse_api_error(reqwest::StatusCode::BAD_REQUEST, "bad"),
            CloudShellError::Aws { .. }
        ));
    }
}

/// Response from DescribeEnvironments operation
#[derive(Debug, Clone)]
pub struct DescribeEnvironmentsResponse {
    pub raw_response: serde_json::Value,
}

impl DescribeEnvironmentsResponse {
    /// Get the list of environments
    pub fn environments(&self) -> Vec<serde_json::Value> {
        self.raw_response
            .get("Environments")
            .and_then(|envs| envs.as_array())
            .cloned()
            .unwrap_or_default()
    }

    /// Pretty-print the response
    pub fn pretty_print(&self) -> std::result::Result<String, serde_json::error::Error> {
        serde_json::to_string_pretty(&self.raw_response)
    }
}

/// Response from CreateEnvironment operation
#[derive(Debug, Clone)]
pub struct CreateEnvironmentResponse {
    pub raw_response: serde_json::Value,
}

impl CreateEnvironmentResponse {
    /// Get the environment ID
    pub fn environment_id(&self) -> Option<String> {
        self.raw_response
            .get("EnvironmentId")
            .and_then(|id| id.as_str())
            .map(|s| s.to_string())
    }

    /// Get the environment status
    pub fn status(&self) -> Option<String> {
        self.raw_response
            .get("Status")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
    }

    /// Pretty-print the response
    pub fn pretty_print(&self) -> std::result::Result<String, serde_json::error::Error> {
        serde_json::to_string_pretty(&self.raw_response)
    }
}

/// Response from StopEnvironment operation
#[derive(Debug, Clone)]
pub struct StopEnvironmentResponse {
    pub raw_response: serde_json::Value,
}

impl StopEnvironmentResponse {
    /// Get the environment ID
    pub fn environment_id(&self) -> Option<String> {
        self.raw_response
            .get("EnvironmentId")
            .and_then(|id| id.as_str())
            .map(|s| s.to_string())
    }

    /// Pretty-print the response
    pub fn pretty_print(&self) -> std::result::Result<String, serde_json::error::Error> {
        serde_json::to_string_pretty(&self.raw_response)
    }
}

/// Response from GetEnvironmentStatus operation
#[derive(Debug, Clone)]
pub struct EnvironmentStatusResponse {
    pub raw_response: serde_json::Value,
}

impl EnvironmentStatusResponse {
    /// Get the environment status
    pub fn status(&self) -> Option<String> {
        self.raw_response
            .get("Status")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
    }

    /// Get the environment ID
    pub fn environment_id(&self) -> Option<String> {
        self.raw_response
            .get("EnvironmentId")
            .and_then(|id| id.as_str())
            .map(|s| s.to_string())
    }

    /// Get the status reason (if any)
    pub fn status_reason(&self) -> Option<String> {
        self.raw_response
            .get("StatusReason")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string())
    }

    /// Pretty-print the response
    pub fn pretty_print(&self) -> std::result::Result<String, serde_json::error::Error> {
        serde_json::to_string_pretty(&self.raw_response)
    }
}

/// Response from CreateSession operation
#[derive(Debug, Clone)]
pub struct CreateSessionResponse {
    pub raw_response: serde_json::Value,
}

impl CreateSessionResponse {
    /// Get the session ID
    pub fn session_id(&self) -> Option<String> {
        self.raw_response
            .get("SessionId")
            .and_then(|id| id.as_str())
            .map(|s| s.to_string())
    }

    /// Get the token value
    pub fn token_value(&self) -> Option<String> {
        self.raw_response
            .get("TokenValue")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
    }

    /// Get the stream URL
    pub fn stream_url(&self) -> Option<String> {
        self.raw_response
            .get("StreamUrl")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
    }

    /// Pretty-print the response
    pub fn pretty_print(&self) -> std::result::Result<String, serde_json::error::Error> {
        serde_json::to_string_pretty(&self.raw_response)
    }
}

/// Response from file transfer operations
#[derive(Debug, Clone)]
pub struct FileTransferResponse {
    pub raw_response: serde_json::Value,
}

impl FileTransferResponse {
    /// Get the file upload presigned URL
    pub fn file_upload_presigned_url(&self) -> Option<String> {
        self.raw_response
            .get("FileUploadPresignedUrl")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
    }

    /// Get the file upload presigned fields
    pub fn file_upload_presigned_fields(&self) -> Option<serde_json::Value> {
        self.raw_response.get("FileUploadPresignedFields").cloned()
    }

    /// Get the file download presigned URL
    pub fn file_download_presigned_url(&self) -> Option<String> {
        self.raw_response
            .get("FileDownloadPresignedUrl")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
    }

    /// Get the file download presigned key
    pub fn file_download_presigned_key(&self) -> Option<String> {
        self.raw_response
            .get("FileDownloadPresignedKey")
            .and_then(|k| k.as_str())
            .map(|s| s.to_string())
    }

    /// Get the file download presigned key hash
    pub fn file_download_presigned_key_hash(&self) -> Option<String> {
        self.raw_response
            .get("FileDownloadPresignedKeyHash")
            .and_then(|h| h.as_str())
            .map(|s| s.to_string())
    }

    /// Pretty-print the response
    pub fn pretty_print(&self) -> std::result::Result<String, serde_json::error::Error> {
        serde_json::to_string_pretty(&self.raw_response)
    }
}
