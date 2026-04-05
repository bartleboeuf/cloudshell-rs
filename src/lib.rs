use aws_config::BehaviorVersion;
use aws_credential_types::provider::ProvideCredentials;
use aws_sigv4::http_request::{
    SignableBody, SignableRequest, SigningParams, SigningSettings, sign,
};
use std::time::SystemTime;

/// CloudShell API client for AWS CloudShell operations
pub struct CloudShellClient {
    region: String,
    credentials: aws_credential_types::Credentials,
}

impl CloudShellClient {
    /// Create a new CloudShell client with credentials from AWS configuration
    pub async fn new(
        region: String,
        profile: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Load AWS configuration with optional profile
        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
        if let Some(profile) = profile {
            config_builder = config_builder.profile_name(&profile);
        }
        let config = config_builder.load().await;

        // Get credentials
        let credentials_provider = config
            .credentials_provider()
            .ok_or("No credentials provider available")?;
        let credentials = credentials_provider.provide_credentials().await?;

        Ok(CloudShellClient {
            region,
            credentials,
        })
    }

    /// Get the current AWS credentials (for manual credential injection)
    pub fn get_credentials(&self) -> &aws_credential_types::Credentials {
        &self.credentials
    }

    /// Describe all CloudShell environments
    pub async fn describe_environments(
        &self,
    ) -> Result<DescribeEnvironmentsResponse, Box<dyn std::error::Error>> {
        let response = self.call_api("describeEnvironments", b"{}").await?;
        let json: serde_json::Value = serde_json::from_str(&response)?;

        Ok(DescribeEnvironmentsResponse { raw_response: json })
    }

    /// Create a new CloudShell environment
    ///
    /// # Arguments
    /// * `environment_name` - Optional name (required for VPC environments)
    /// * `vpc_config` - Optional VPC configuration (None = public environment)
    ///
    /// # Example: Public environment
    /// ```
    /// let response = client.create_environment(None, None).await?;
    /// ```
    ///
    /// # Example: VPC environment
    /// ```
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
    ) -> Result<CreateEnvironmentResponse, Box<dyn std::error::Error>> {
        let mut body = serde_json::json!({});
        if let Some(name) = environment_name {
            body["EnvironmentName"] = serde_json::json!(name);
        }
        if let Some(vpc) = vpc_config {
            body["VpcConfig"] = serde_json::json!({
                "VpcId": vpc.vpc_id,
                "SubnetIds": vpc.subnet_ids,
                "SecurityGroupIds": vpc.security_group_ids,
            });
        }

        let response = self
            .call_api("createEnvironment", body.to_string().as_bytes())
            .await?;
        let json: serde_json::Value = serde_json::from_str(&response)?;

        Ok(CreateEnvironmentResponse { raw_response: json })
    }

    /// Start a CloudShell environment
    pub async fn start_environment(
        &self,
        environment_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        self.call_api("startEnvironment", body.to_string().as_bytes())
            .await?;
        Ok(())
    }

    /// Stop a CloudShell environment
    pub async fn stop_environment(
        &self,
        environment_id: &str,
    ) -> Result<StopEnvironmentResponse, Box<dyn std::error::Error>> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        let response = self
            .call_api("stopEnvironment", body.to_string().as_bytes())
            .await?;
        let json: serde_json::Value = serde_json::from_str(&response)?;

        Ok(StopEnvironmentResponse { raw_response: json })
    }

    /// Get the status of a CloudShell environment
    pub async fn get_environment_status(
        &self,
        environment_id: &str,
    ) -> Result<EnvironmentStatusResponse, Box<dyn std::error::Error>> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        let response = self
            .call_api("getEnvironmentStatus", body.to_string().as_bytes())
            .await?;
        let json: serde_json::Value = serde_json::from_str(&response)?;

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
    /// ```
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
    ) -> Result<CreateSessionResponse, Box<dyn std::error::Error>> {
        // Validate tab_id is a valid UUID
        if uuid::Uuid::parse_str(tab_id).is_err() {
            return Err(format!("Invalid TabId: '{}' is not a valid UUID", tab_id).into());
        }

        let mut body = serde_json::json!({
            "EnvironmentId": environment_id,
            "SessionType": session_type,
            "TabId": tab_id,
        });
        if let Some(disabled) = q_cli_disabled {
            body["QCliDisabled"] = serde_json::json!(disabled);
        }

        let response = self
            .call_api("createSession", body.to_string().as_bytes())
            .await?;
        let json: serde_json::Value = serde_json::from_str(&response)?;

        Ok(CreateSessionResponse { raw_response: json })
    }

    /// Send a heartbeat to keep a CloudShell environment alive
    pub async fn send_heart_beat(
        &self,
        environment_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        self.call_api("sendHeartBeat", body.to_string().as_bytes())
            .await?;
        Ok(())
    }

    /// Delete a CloudShell session
    pub async fn delete_session(
        &self,
        environment_id: &str,
        session_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let body = serde_json::json!({
            "EnvironmentId": environment_id,
            "SessionId": session_id,
        });
        self.call_api("deleteSession", body.to_string().as_bytes())
            .await?;
        Ok(())
    }

    /// Delete a CloudShell environment
    pub async fn delete_environment(
        &self,
        environment_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        self.call_api("deleteEnvironment", body.to_string().as_bytes())
            .await?;
        Ok(())
    }

    /// Get file upload URLs
    pub async fn get_file_upload_urls(
        &self,
        environment_id: &str,
    ) -> Result<FileTransferResponse, Box<dyn std::error::Error>> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        let response = self
            .call_api("getFileUploadUrls", body.to_string().as_bytes())
            .await?;
        let json: serde_json::Value = serde_json::from_str(&response)?;

        Ok(FileTransferResponse { raw_response: json })
    }

    /// Get file download URLs
    pub async fn get_file_download_urls(
        &self,
        environment_id: &str,
    ) -> Result<FileTransferResponse, Box<dyn std::error::Error>> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        let response = self
            .call_api("getFileDownloadUrls", body.to_string().as_bytes())
            .await?;
        let json: serde_json::Value = serde_json::from_str(&response)?;

        Ok(FileTransferResponse { raw_response: json })
    }

    /// Put credentials into a CloudShell environment (console session tokens only)
    pub async fn put_credentials(
        &self,
        environment_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let body = serde_json::json!({ "EnvironmentId": environment_id });
        self.call_api("putCredentials", body.to_string().as_bytes())
            .await?;
        Ok(())
    }

    /// Call a CloudShell API operation
    async fn call_api(
        &self,
        operation: &str,
        body: &[u8],
    ) -> Result<String, Box<dyn std::error::Error>> {
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
            SignableBody::Bytes(body),
        )?;

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
                .build()?,
        );
        let signing_result = sign(signable_request, &signing_params)?;
        let signing_instructions = signing_result.output();

        // Build final request with all headers
        let mut final_headers = reqwest::header::HeaderMap::new();

        // Add Content-Type header
        final_headers.insert(
            "Content-Type".parse::<reqwest::header::HeaderName>()?,
            "application/x-amz-json-1.1".parse::<reqwest::header::HeaderValue>()?,
        );

        // Add SigV4 signed headers
        for (name, value) in signing_instructions.headers() {
            final_headers.insert(
                name.parse::<reqwest::header::HeaderName>()?,
                value.parse::<reqwest::header::HeaderValue>()?,
            );
        }

        // Make HTTP POST request
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .headers(final_headers)
            .body(body.to_vec())
            .send()
            .await?;

        // Handle response
        let status = response.status();
        let response_body = response.text().await?;

        if status.is_success() {
            Ok(response_body)
        } else {
            Err(format!("CloudShell API error: {} - {}", status, response_body).into())
        }
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
    /// # Panics
    /// Panics if more than 5 security groups are provided
    ///
    /// # Example
    /// ```
    /// let vpc = VpcConfig::new(
    ///     "vpc-123".to_string(),
    ///     vec!["subnet-1".to_string()],
    ///     vec!["sg-1".to_string()],
    /// );
    /// ```
    pub fn new(
        vpc_id: String,
        subnet_ids: Vec<String>,
        security_group_ids: Vec<String>,
    ) -> Self {
        if security_group_ids.len() > 5 {
            panic!(
                "Max 5 security groups allowed, got {}",
                security_group_ids.len()
            );
        }
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
    /// let vpc = VpcConfig {
    ///     vpc_id: "vpc-123".to_string(),
    ///     subnet_ids: vec!["subnet-1".to_string()],
    ///     security_group_ids: vec!["sg-1".to_string(); 6],  // Too many
    /// };
    /// assert!(vpc.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        if self.security_group_ids.len() > 5 {
            return Err(format!(
                "Max 5 security groups allowed, got {}",
                self.security_group_ids.len()
            ));
        }
        if self.subnet_ids.is_empty() {
            return Err("At least 1 subnet is required".to_string());
        }
        Ok(())
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
    pub fn pretty_print(&self) -> Result<String, serde_json::error::Error> {
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
    pub fn pretty_print(&self) -> Result<String, serde_json::error::Error> {
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
    pub fn pretty_print(&self) -> Result<String, serde_json::error::Error> {
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
    pub fn pretty_print(&self) -> Result<String, serde_json::error::Error> {
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
    pub fn pretty_print(&self) -> Result<String, serde_json::error::Error> {
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
    pub fn pretty_print(&self) -> Result<String, serde_json::error::Error> {
        serde_json::to_string_pretty(&self.raw_response)
    }
}
