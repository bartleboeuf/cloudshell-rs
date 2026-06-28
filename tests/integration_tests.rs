use cloudshell_rs::{
    CreateEnvironmentResponse, CreateSessionResponse, DescribeEnvironmentsResponse,
    EnvironmentStatusResponse, FileTransferResponse, StopEnvironmentResponse, VpcConfig,
};
use serde_json::json;

// ============================================================================
// DescribeEnvironmentsResponse Tests
// ============================================================================

#[test]
fn test_describe_environments_response_empty() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({ "Environments": [] }),
    };

    assert_eq!(response.environments().len(), 0);
}

#[test]
fn test_describe_environments_response_with_envs() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({
            "Environments": [
                { "EnvironmentId": "env-1", "Status": "RUNNING" },
                { "EnvironmentId": "env-2", "Status": "SUSPENDED" }
            ]
        }),
    };

    let envs = response.environments();
    assert_eq!(envs.len(), 2);
    assert_eq!(envs[0]["EnvironmentId"].as_str(), Some("env-1"));
    assert_eq!(envs[1]["EnvironmentId"].as_str(), Some("env-2"));
}

#[test]
fn test_describe_environments_response_pretty_print() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({
            "Environments": [
                { "EnvironmentId": "env-123", "Status": "RUNNING" }
            ]
        }),
    };

    let output = response.pretty_print();
    assert!(output.is_ok());
    let pretty = output.unwrap();
    assert!(pretty.contains("env-123"));
    assert!(pretty.contains("RUNNING"));
}

#[test]
fn test_describe_environments_response_missing_environments() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({}),
    };

    assert_eq!(response.environments().len(), 0);
}

// ============================================================================
// CreateEnvironmentResponse Tests
// ============================================================================

#[test]
fn test_create_environment_response_success() {
    let response = CreateEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": "env-abc-123",
            "Status": "CREATING"
        }),
    };

    assert_eq!(response.environment_id(), Some("env-abc-123".to_string()));
    assert_eq!(response.status(), Some("CREATING".to_string()));
}

#[test]
fn test_create_environment_response_missing_fields() {
    let response = CreateEnvironmentResponse {
        raw_response: json!({}),
    };

    assert_eq!(response.environment_id(), None);
    assert_eq!(response.status(), None);
}

#[test]
fn test_create_environment_response_pretty_print() {
    let response = CreateEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": "env-xyz",
            "Status": "RUNNING"
        }),
    };

    let output = response.pretty_print();
    assert!(output.is_ok());
    let pretty = output.unwrap();
    assert!(pretty.contains("env-xyz"));
}

// ============================================================================
// StopEnvironmentResponse Tests
// ============================================================================

#[test]
fn test_stop_environment_response() {
    let response = StopEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": "env-stop-123"
        }),
    };

    assert_eq!(response.environment_id(), Some("env-stop-123".to_string()));
}

#[test]
fn test_stop_environment_response_pretty_print() {
    let response = StopEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": "env-stop"
        }),
    };

    let output = response.pretty_print();
    assert!(output.is_ok());
    assert!(output.unwrap().contains("env-stop"));
}

// ============================================================================
// EnvironmentStatusResponse Tests
// ============================================================================

#[test]
fn test_environment_status_response_full() {
    let response = EnvironmentStatusResponse {
        raw_response: json!({
            "Status": "RUNNING",
            "EnvironmentId": "env-123",
            "StatusReason": "Environment is active"
        }),
    };

    assert_eq!(response.status(), Some("RUNNING".to_string()));
    assert_eq!(response.environment_id(), Some("env-123".to_string()));
    assert_eq!(
        response.status_reason(),
        Some("Environment is active".to_string())
    );
}

#[test]
fn test_environment_status_response_without_reason() {
    let response = EnvironmentStatusResponse {
        raw_response: json!({
            "Status": "RUNNING",
            "EnvironmentId": "env-123"
        }),
    };

    assert_eq!(response.status(), Some("RUNNING".to_string()));
    assert_eq!(response.environment_id(), Some("env-123".to_string()));
    assert_eq!(response.status_reason(), None);
}

#[test]
fn test_environment_status_response_suspended() {
    let response = EnvironmentStatusResponse {
        raw_response: json!({
            "Status": "SUSPENDED",
            "EnvironmentId": "env-456",
            "StatusReason": "Inactive"
        }),
    };

    assert_eq!(response.status(), Some("SUSPENDED".to_string()));
}

#[test]
fn test_environment_status_response_pretty_print() {
    let response = EnvironmentStatusResponse {
        raw_response: json!({
            "Status": "RUNNING",
            "EnvironmentId": "env-123",
            "StatusReason": "Active"
        }),
    };

    let output = response.pretty_print();
    assert!(output.is_ok());
    let pretty = output.unwrap();
    assert!(pretty.contains("RUNNING"));
    assert!(pretty.contains("env-123"));
}

// ============================================================================
// CreateSessionResponse Tests
// ============================================================================

#[test]
fn test_create_session_response_full() {
    let response = CreateSessionResponse {
        raw_response: json!({
            "SessionId": "sess-123",
            "TokenValue": "token-abc",
            "StreamUrl": "wss://ssmmessages.us-east-1.amazonaws.com/v1/data-channel/sess-123"
        }),
    };

    assert_eq!(response.session_id(), Some("sess-123".to_string()));
    assert_eq!(response.token_value(), Some("token-abc".to_string()));
    assert!(response.stream_url().unwrap().contains("ssmmessages"));
}

#[test]
fn test_create_session_response_missing_fields() {
    let response = CreateSessionResponse {
        raw_response: json!({}),
    };

    assert_eq!(response.session_id(), None);
    assert_eq!(response.token_value(), None);
    assert_eq!(response.stream_url(), None);
}

#[test]
fn test_create_session_response_pretty_print() {
    let response = CreateSessionResponse {
        raw_response: json!({
            "SessionId": "sess-xyz",
            "TokenValue": "token-123",
            "StreamUrl": "wss://example.com"
        }),
    };

    let output = response.pretty_print();
    assert!(output.is_ok());
    let pretty = output.unwrap();
    assert!(pretty.contains("sess-xyz"));
    assert!(pretty.contains("token-123"));
}

// ============================================================================
// FileTransferResponse Tests
// ============================================================================

#[test]
fn test_file_upload_response() {
    let response = FileTransferResponse {
        raw_response: json!({
            "FileUploadPresignedUrl": "https://s3.amazonaws.com/upload",
            "FileUploadPresignedFields": {
                "key": "file-uuid",
                "bucket": "my-bucket",
                "X-Amz-Algorithm": "AWS4-HMAC-SHA256"
            }
        }),
    };

    assert_eq!(
        response.file_upload_presigned_url(),
        Some("https://s3.amazonaws.com/upload".to_string())
    );

    let fields = response.file_upload_presigned_fields();
    assert!(fields.is_some());
    let fields = fields.unwrap();
    assert_eq!(fields["key"].as_str(), Some("file-uuid"));
}

#[test]
fn test_file_download_response() {
    let response = FileTransferResponse {
        raw_response: json!({
            "FileDownloadPresignedUrl": "https://s3.amazonaws.com/download",
            "FileDownloadPresignedKey": "file-key-123",
            "FileDownloadPresignedKeyHash": "hash-abc"
        }),
    };

    assert_eq!(
        response.file_download_presigned_url(),
        Some("https://s3.amazonaws.com/download".to_string())
    );
    assert_eq!(
        response.file_download_presigned_key(),
        Some("file-key-123".to_string())
    );
    assert_eq!(
        response.file_download_presigned_key_hash(),
        Some("hash-abc".to_string())
    );
}

#[test]
fn test_file_transfer_response_missing_fields() {
    let response = FileTransferResponse {
        raw_response: json!({}),
    };

    assert_eq!(response.file_upload_presigned_url(), None);
    assert_eq!(response.file_upload_presigned_fields(), None);
    assert_eq!(response.file_download_presigned_url(), None);
    assert_eq!(response.file_download_presigned_key(), None);
    assert_eq!(response.file_download_presigned_key_hash(), None);
}

#[test]
fn test_file_transfer_response_pretty_print() {
    let response = FileTransferResponse {
        raw_response: json!({
            "FileUploadPresignedUrl": "https://s3.amazonaws.com/upload",
            "FileDownloadPresignedUrl": "https://s3.amazonaws.com/download"
        }),
    };

    let output = response.pretty_print();
    assert!(output.is_ok());
    let pretty = output.unwrap();
    assert!(pretty.contains("FileUploadPresignedUrl"));
    assert!(pretty.contains("FileDownloadPresignedUrl"));
}

// ============================================================================
// VpcConfig Tests
// ============================================================================

#[test]
fn test_vpc_config_creation() {
    let vpc = VpcConfig {
        vpc_id: "vpc-123".to_string(),
        subnet_ids: vec!["subnet-1".to_string(), "subnet-2".to_string()],
        security_group_ids: vec!["sg-1".to_string()],
    };

    assert_eq!(vpc.vpc_id, "vpc-123");
    assert_eq!(vpc.subnet_ids.len(), 2);
    assert_eq!(vpc.security_group_ids.len(), 1);
}

#[test]
fn test_vpc_config_clone() {
    let vpc = VpcConfig {
        vpc_id: "vpc-abc".to_string(),
        subnet_ids: vec!["subnet-1".to_string()],
        security_group_ids: vec!["sg-1".to_string()],
    };

    let vpc_clone = vpc.clone();
    assert_eq!(vpc_clone.vpc_id, vpc.vpc_id);
    assert_eq!(vpc_clone.subnet_ids, vpc.subnet_ids);
}

// ============================================================================
// Response Clone Tests
// ============================================================================

#[test]
fn test_describe_environments_response_clone() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({ "Environments": [] }),
    };

    let cloned = response.clone();
    assert_eq!(cloned.environments().len(), 0);
}

#[test]
fn test_create_environment_response_clone() {
    let response = CreateEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": "env-123",
            "Status": "CREATING"
        }),
    };

    let cloned = response.clone();
    assert_eq!(cloned.environment_id(), Some("env-123".to_string()));
}

#[test]
fn test_environment_status_response_clone() {
    let response = EnvironmentStatusResponse {
        raw_response: json!({
            "Status": "RUNNING",
            "EnvironmentId": "env-123"
        }),
    };

    let cloned = response.clone();
    assert_eq!(cloned.status(), Some("RUNNING".to_string()));
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[test]
fn test_response_with_null_values() {
    let response = CreateEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": null,
            "Status": null
        }),
    };

    assert_eq!(response.environment_id(), None);
    assert_eq!(response.status(), None);
}

#[test]
fn test_response_with_non_string_values() {
    let response = EnvironmentStatusResponse {
        raw_response: json!({
            "Status": 123,  // wrong type
            "EnvironmentId": { "nested": "object" }  // wrong type
        }),
    };

    assert_eq!(response.status(), None);
    assert_eq!(response.environment_id(), None);
}

#[test]
fn test_file_transfer_response_with_empty_fields() {
    let response = FileTransferResponse {
        raw_response: json!({
            "FileUploadPresignedUrl": "",
            "FileUploadPresignedFields": {},
            "FileDownloadPresignedUrl": ""
        }),
    };

    assert_eq!(response.file_upload_presigned_url(), Some("".to_string()));
    assert_eq!(response.file_download_presigned_url(), Some("".to_string()));
}

#[test]
fn test_create_session_response_with_empty_strings() {
    let response = CreateSessionResponse {
        raw_response: json!({
            "SessionId": "",
            "TokenValue": "",
            "StreamUrl": ""
        }),
    };

    assert_eq!(response.session_id(), Some("".to_string()));
    assert_eq!(response.token_value(), Some("".to_string()));
    assert_eq!(response.stream_url(), Some("".to_string()));
}

#[test]
fn test_environments_response_maintains_order() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({
            "Environments": [
                { "EnvironmentId": "env-1" },
                { "EnvironmentId": "env-2" },
                { "EnvironmentId": "env-3" }
            ]
        }),
    };

    let envs = response.environments();
    assert_eq!(envs[0]["EnvironmentId"].as_str(), Some("env-1"));
    assert_eq!(envs[1]["EnvironmentId"].as_str(), Some("env-2"));
    assert_eq!(envs[2]["EnvironmentId"].as_str(), Some("env-3"));
}

#[test]
fn test_file_transfer_with_complex_presigned_fields() {
    let response = FileTransferResponse {
        raw_response: json!({
            "FileUploadPresignedUrl": "https://s3.amazonaws.com/upload",
            "FileUploadPresignedFields": {
                "key": "uploads/12345/file.txt",
                "bucket": "my-cloudshell-bucket",
                "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
                "X-Amz-Credential": "AKIAIOSFODNN7EXAMPLE/20260403/us-east-1/s3/aws4_request",
                "X-Amz-Date": "20260403T203137Z",
                "X-Amz-Security-Token": "token...",
                "Policy": "policy...",
                "X-Amz-Signature": "signature...",
                "x-amz-server-side-encryption-customer-algorithm": "AES256"
            }
        }),
    };

    let fields = response.file_upload_presigned_fields();
    assert!(fields.is_some());
    let fields = fields.unwrap();
    assert_eq!(
        fields.get("key").and_then(|k| k.as_str()),
        Some("uploads/12345/file.txt")
    );
    assert_eq!(
        fields.get("bucket").and_then(|b| b.as_str()),
        Some("my-cloudshell-bucket")
    );
    assert!(fields.get("X-Amz-Algorithm").is_some());
}

// ============================================================================
// Raw Response Field Access Tests
// ============================================================================

#[test]
fn test_describe_environments_raw_response_access() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({
            "Environments": [
                { "EnvironmentId": "env-1", "Status": "RUNNING" }
            ],
            "ResponseMetadata": {
                "RequestId": "req-123"
            }
        }),
    };

    // Direct access to raw response should work
    assert!(response.raw_response.get("ResponseMetadata").is_some());
    assert_eq!(
        response.raw_response["ResponseMetadata"]["RequestId"].as_str(),
        Some("req-123")
    );
}

#[test]
fn test_create_environment_response_all_fields() {
    let response = CreateEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": "env-full-123",
            "Status": "CREATING",
            "ConnectionType": "STANDARD",
            "AwsAccountId": "123456789012",
            "Arn": "arn:aws:cloudshell:us-east-1:123456789012:environment/env-full-123"
        }),
    };

    assert_eq!(response.environment_id(), Some("env-full-123".to_string()));
    assert_eq!(response.status(), Some("CREATING".to_string()));
    // Verify raw access to additional fields
    assert_eq!(
        response
            .raw_response
            .get("ConnectionType")
            .and_then(|v| v.as_str()),
        Some("STANDARD")
    );
}

#[test]
fn test_environment_status_response_all_fields() {
    let response = EnvironmentStatusResponse {
        raw_response: json!({
            "Status": "RUNNING",
            "EnvironmentId": "env-full-123",
            "StatusReason": "Environment is active",
            "ConnectionType": "STANDARD",
            "AwsAccountId": "123456789012"
        }),
    };

    assert_eq!(response.status(), Some("RUNNING".to_string()));
    assert_eq!(response.environment_id(), Some("env-full-123".to_string()));
    assert_eq!(
        response.status_reason(),
        Some("Environment is active".to_string())
    );
}

#[test]
fn test_create_session_response_all_fields() {
    let response = CreateSessionResponse {
        raw_response: json!({
            "SessionId": "sess-full-123",
            "TokenValue": "token-full-abc",
            "StreamUrl": "wss://ssmmessages.us-east-1.amazonaws.com/v1/data-channel/sess-full-123",
            "Credentials": {
                "AccessKeyId": "AKIA...",
                "SecretAccessKey": "...",
                "SessionToken": "..."
            },
            "ExpirationTime": "2026-04-05T10:30:00Z"
        }),
    };

    assert_eq!(response.session_id(), Some("sess-full-123".to_string()));
    assert_eq!(response.token_value(), Some("token-full-abc".to_string()));
    assert!(response.stream_url().unwrap().contains("ssmmessages"));
    // Verify raw access to additional fields
    assert!(response.raw_response.get("Credentials").is_some());
}

// ============================================================================
// Debug Trait Tests (verify derived traits work)
// ============================================================================

#[test]
fn test_describe_environments_response_debug() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({ "Environments": [] }),
    };

    let debug_str = format!("{:?}", response);
    assert!(debug_str.contains("DescribeEnvironmentsResponse"));
    assert!(debug_str.contains("Environments"));
}

#[test]
fn test_create_environment_response_debug() {
    let response = CreateEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": "env-123",
            "Status": "CREATING"
        }),
    };

    let debug_str = format!("{:?}", response);
    assert!(debug_str.contains("CreateEnvironmentResponse"));
}

#[test]
fn test_vpc_config_debug() {
    let vpc = VpcConfig {
        vpc_id: "vpc-123".to_string(),
        subnet_ids: vec!["subnet-1".to_string()],
        security_group_ids: vec!["sg-1".to_string()],
    };

    let debug_str = format!("{:?}", vpc);
    assert!(debug_str.contains("vpc-123"));
    assert!(debug_str.contains("subnet-1"));
}

// ============================================================================
// VpcConfig Tests - Additional Coverage
// ============================================================================

#[test]
fn test_vpc_config_new_valid() {
    let vpc = VpcConfig::new(
        "vpc-123".to_string(),
        vec!["subnet-1".to_string()],
        vec!["sg-1".to_string()],
    );

    assert_eq!(vpc.vpc_id, "vpc-123");
    assert_eq!(vpc.subnet_ids.len(), 1);
    assert_eq!(vpc.security_group_ids.len(), 1);
}

#[test]
fn test_vpc_config_validate_valid() {
    let vpc = VpcConfig {
        vpc_id: "vpc-123".to_string(),
        subnet_ids: vec!["subnet-1".to_string()],
        security_group_ids: vec!["sg-1".to_string()],
    };

    assert!(vpc.validate().is_ok());
}

#[test]
fn test_vpc_config_validate_max_security_groups() {
    let vpc = VpcConfig {
        vpc_id: "vpc-123".to_string(),
        subnet_ids: vec!["subnet-1".to_string()],
        security_group_ids: vec![
            "sg-1".to_string(),
            "sg-2".to_string(),
            "sg-3".to_string(),
            "sg-4".to_string(),
            "sg-5".to_string(),
        ],
    };

    assert!(vpc.validate().is_ok());
}

#[test]
fn test_vpc_config_validate_too_many_security_groups() {
    let vpc = VpcConfig {
        vpc_id: "vpc-123".to_string(),
        subnet_ids: vec!["subnet-1".to_string()],
        security_group_ids: vec![
            "sg-1".to_string(),
            "sg-2".to_string(),
            "sg-3".to_string(),
            "sg-4".to_string(),
            "sg-5".to_string(),
            "sg-6".to_string(),
        ],
    };

    let result = vpc.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Max 5 security groups"));
}

#[test]
fn test_vpc_config_validate_no_subnets() {
    let vpc = VpcConfig {
        vpc_id: "vpc-123".to_string(),
        subnet_ids: vec![],
        security_group_ids: vec!["sg-1".to_string()],
    };

    let result = vpc.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("subnet"));
}

#[test]
#[should_panic(expected = "Max 5 security groups")]
fn test_vpc_config_new_panics_on_too_many_security_groups() {
    VpcConfig::new(
        "vpc-123".to_string(),
        vec!["subnet-1".to_string()],
        vec![
            "sg-1".to_string(),
            "sg-2".to_string(),
            "sg-3".to_string(),
            "sg-4".to_string(),
            "sg-5".to_string(),
            "sg-6".to_string(),
        ],
    );
}

#[test]
fn test_vpc_config_with_multiple_subnets() {
    let vpc = VpcConfig {
        vpc_id: "vpc-abc-def".to_string(),
        subnet_ids: vec![
            "subnet-1".to_string(),
            "subnet-2".to_string(),
            "subnet-3".to_string(),
        ],
        security_group_ids: vec!["sg-1".to_string(), "sg-2".to_string()],
    };

    assert_eq!(vpc.subnet_ids.len(), 3);
    assert_eq!(vpc.security_group_ids.len(), 2);
    assert!(vpc.subnet_ids.contains(&"subnet-2".to_string()));
}

#[test]
fn test_vpc_config_empty_lists() {
    let vpc = VpcConfig {
        vpc_id: "vpc-empty".to_string(),
        subnet_ids: vec![],
        security_group_ids: vec![],
    };

    assert_eq!(vpc.vpc_id, "vpc-empty");
    assert!(vpc.subnet_ids.is_empty());
    assert!(vpc.security_group_ids.is_empty());
}

#[test]
fn test_vpc_config_with_long_ids() {
    let vpc = VpcConfig {
        vpc_id: "vpc-0123456789abcdef0123456789abcdef".to_string(),
        subnet_ids: vec!["subnet-0123456789abcdef0123456789abcdef".to_string()],
        security_group_ids: vec!["sg-0123456789abcdef0123456789abcdef".to_string()],
    };

    assert_eq!(vpc.vpc_id.len(), 36);
    assert_eq!(vpc.subnet_ids[0].len(), 39);
    assert_eq!(vpc.security_group_ids[0].len(), 35);
}

// ============================================================================
// Multiple Environments Tests
// ============================================================================

#[test]
fn test_describe_environments_with_various_statuses() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({
            "Environments": [
                { "EnvironmentId": "env-1", "Status": "RUNNING" },
                { "EnvironmentId": "env-2", "Status": "SUSPENDED" },
                { "EnvironmentId": "env-3", "Status": "PENDING" },
                { "EnvironmentId": "env-4", "Status": "CREATING" }
            ]
        }),
    };

    let envs = response.environments();
    assert_eq!(envs.len(), 4);

    // Verify each has the expected status
    let statuses: Vec<String> = envs
        .iter()
        .map(|e| {
            e.get("Status")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();

    assert_eq!(
        statuses,
        vec!["RUNNING", "SUSPENDED", "PENDING", "CREATING"]
    );
}

#[test]
fn test_describe_environments_large_list() {
    let mut envs = vec![];
    for i in 0..100 {
        envs.push(json!({
            "EnvironmentId": format!("env-{}", i),
            "Status": if i % 2 == 0 { "RUNNING" } else { "SUSPENDED" }
        }));
    }

    let response = DescribeEnvironmentsResponse {
        raw_response: json!({ "Environments": envs }),
    };

    let result = response.environments();
    assert_eq!(result.len(), 100);
}

// ============================================================================
// Unicode and Special Characters
// ============================================================================

#[test]
fn test_response_with_unicode_characters() {
    let response = EnvironmentStatusResponse {
        raw_response: json!({
            "Status": "RUNNING",
            "EnvironmentId": "env-123",
            "StatusReason": "Environment is active 🚀 ✨"
        }),
    };

    let reason = response.status_reason();
    assert!(reason.is_some());
    assert!(reason.unwrap().contains("🚀"));
}

#[test]
fn test_response_with_special_characters_in_url() {
    let response = CreateSessionResponse {
        raw_response: json!({
            "SessionId": "sess-123",
            "TokenValue": "token-with-special-chars-!@#$%",
            "StreamUrl": "wss://example.com/path?param=value&other=123"
        }),
    };

    assert!(response.token_value().unwrap().contains("!@#$%"));
    assert!(response.stream_url().unwrap().contains("param=value"));
}

// ============================================================================
// Complex JSON Nesting
// ============================================================================

#[test]
fn test_file_transfer_with_nested_metadata() {
    let response = FileTransferResponse {
        raw_response: json!({
            "FileUploadPresignedUrl": "https://s3.amazonaws.com/upload",
            "FileUploadPresignedFields": {
                "key": "file-uuid",
                "bucket": "my-bucket",
                "Metadata": {
                    "owner": "user-123",
                    "environment": "prod",
                    "tags": ["important", "backup"]
                }
            }
        }),
    };

    let fields = response.file_upload_presigned_fields().unwrap();
    assert_eq!(fields["key"].as_str(), Some("file-uuid"));
    assert!(fields.get("Metadata").is_some());
}

// ============================================================================
// Response Serialization Round-Trip Tests
// ============================================================================

#[test]
fn test_response_serialization_round_trip() {
    let original = DescribeEnvironmentsResponse {
        raw_response: json!({
            "Environments": [
                { "EnvironmentId": "env-1", "Status": "RUNNING" },
                { "EnvironmentId": "env-2", "Status": "SUSPENDED" }
            ]
        }),
    };

    // Serialize to string
    let serialized = original.pretty_print().unwrap();

    // Deserialize back
    let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    let recovered = DescribeEnvironmentsResponse {
        raw_response: deserialized,
    };

    // Verify structure is preserved
    assert_eq!(
        original.environments().len(),
        recovered.environments().len()
    );
    assert_eq!(
        original.environments()[0]["EnvironmentId"].as_str(),
        recovered.environments()[0]["EnvironmentId"].as_str()
    );
}

// ============================================================================
// Stop/Delete Operation Responses
// ============================================================================

#[test]
fn test_stop_environment_response_missing_environment_id() {
    let response = StopEnvironmentResponse {
        raw_response: json!({}),
    };

    assert_eq!(response.environment_id(), None);
}

#[test]
fn test_stop_environment_response_with_extra_fields() {
    let response = StopEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": "env-stop-123",
            "Status": "STOPPED",
            "StoppedTime": "2026-04-05T10:00:00Z"
        }),
    };

    assert_eq!(response.environment_id(), Some("env-stop-123".to_string()));
    // Verify extra fields are accessible via raw response
    assert_eq!(
        response.raw_response.get("Status").and_then(|s| s.as_str()),
        Some("STOPPED")
    );
}

// ============================================================================
// Field Combination Tests
// ============================================================================

#[test]
fn test_file_transfer_response_upload_and_download_same_response() {
    let response = FileTransferResponse {
        raw_response: json!({
            "FileUploadPresignedUrl": "https://s3.amazonaws.com/upload",
            "FileUploadPresignedFields": {
                "key": "uploads/file.txt",
                "bucket": "my-bucket"
            },
            "FileDownloadPresignedUrl": "https://s3.amazonaws.com/download",
            "FileDownloadPresignedKey": "downloads/file.txt",
            "FileDownloadPresignedKeyHash": "abc123hash"
        }),
    };

    assert!(response.file_upload_presigned_url().is_some());
    assert!(response.file_upload_presigned_fields().is_some());
    assert!(response.file_download_presigned_url().is_some());
    assert!(response.file_download_presigned_key().is_some());
    assert!(response.file_download_presigned_key_hash().is_some());
}

// ============================================================================
// Array Element Access Tests
// ============================================================================

#[test]
fn test_describe_environments_access_specific_element() {
    let response = DescribeEnvironmentsResponse {
        raw_response: json!({
            "Environments": [
                { "EnvironmentId": "env-first", "Status": "RUNNING" },
                { "EnvironmentId": "env-second", "Status": "SUSPENDED" },
                { "EnvironmentId": "env-third", "Status": "RUNNING" }
            ]
        }),
    };

    let envs = response.environments();
    assert_eq!(envs[0]["EnvironmentId"].as_str(), Some("env-first"));
    assert_eq!(envs[1]["EnvironmentId"].as_str(), Some("env-second"));
    assert_eq!(envs[2]["EnvironmentId"].as_str(), Some("env-third"));
}

// ============================================================================
// Response Type Integration Tests
// ============================================================================

#[test]
fn test_multiple_response_types_independent() {
    let desc_resp = DescribeEnvironmentsResponse {
        raw_response: json!({ "Environments": [] }),
    };

    let create_resp = CreateEnvironmentResponse {
        raw_response: json!({ "EnvironmentId": "env-123" }),
    };

    let status_resp = EnvironmentStatusResponse {
        raw_response: json!({ "Status": "RUNNING" }),
    };

    // Verify they're independent
    assert_eq!(desc_resp.environments().len(), 0);
    assert_eq!(create_resp.environment_id(), Some("env-123".to_string()));
    assert_eq!(status_resp.status(), Some("RUNNING".to_string()));
}

// ============================================================================
// Whitespace and Formatting Tests
// ============================================================================

#[test]
fn test_response_fields_with_extra_whitespace() {
    let response = EnvironmentStatusResponse {
        raw_response: json!({
            "Status": "  RUNNING  ",
            "EnvironmentId": "  env-123  ",
            "StatusReason": "  Active  "
        }),
    };

    // Verify values are returned as-is (including whitespace)
    assert_eq!(response.status().unwrap(), "  RUNNING  ");
}

#[test]
fn test_response_empty_vs_null_distinction() {
    let empty_response = CreateEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": "",
            "Status": ""
        }),
    };

    let null_response = CreateEnvironmentResponse {
        raw_response: json!({
            "EnvironmentId": null,
            "Status": null
        }),
    };

    // Empty strings should still return Some("")
    assert_eq!(empty_response.environment_id(), Some("".to_string()));
    // Nulls should return None
    assert_eq!(null_response.environment_id(), None);
}

// ============================================================================
// Session Creation Tab ID Validation Tests
// ============================================================================

#[test]
fn test_valid_uuid_parsing() {
    // These are valid UUID v4 formats
    let valid_uuids = vec![
        "550e8400-e29b-41d4-a716-446655440000",
        "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
        "00000000-0000-0000-0000-000000000000",
    ];

    for uuid_str in valid_uuids {
        assert!(
            uuid::Uuid::parse_str(uuid_str).is_ok(),
            "Expected {} to be a valid UUID",
            uuid_str
        );
    }
}

#[test]
fn test_invalid_uuid_formats() {
    // These are invalid UUID formats
    let invalid_uuids = vec![
        "not-a-uuid",
        "550e8400-e29b-41d4-a716",                    // Too short
        "550e8400-e29b-41d4-a716-446655440000-extra", // Too long
        "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
        "",
        "not-really-a-uuid-at-all",
    ];

    for uuid_str in invalid_uuids {
        assert!(
            uuid::Uuid::parse_str(uuid_str).is_err(),
            "Expected {} to be invalid",
            uuid_str
        );
    }
}

// ============================================================================
// File Upload/Download Field Extraction
// ============================================================================

#[test]
fn test_file_transfer_presigned_fields_access_individual_keys() {
    let response = FileTransferResponse {
        raw_response: json!({
            "FileUploadPresignedFields": {
                "key": "file-key",
                "bucket": "my-bucket",
                "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
                "X-Amz-Credential": "AKIA123/20260405/us-east-1/s3/aws4_request",
                "X-Amz-Date": "20260405T120000Z",
                "X-Amz-Security-Token": "token-xyz",
                "Policy": "eyJ...",
                "X-Amz-Signature": "sig-abc"
            }
        }),
    };

    let fields = response.file_upload_presigned_fields().unwrap();
    assert_eq!(fields["key"].as_str(), Some("file-key"));
    assert_eq!(fields["bucket"].as_str(), Some("my-bucket"));
    assert_eq!(fields["X-Amz-Algorithm"].as_str(), Some("AWS4-HMAC-SHA256"));
    assert_eq!(
        fields.get("X-Amz-Credential").and_then(|v| v.as_str()),
        Some("AKIA123/20260405/us-east-1/s3/aws4_request")
    );
}
