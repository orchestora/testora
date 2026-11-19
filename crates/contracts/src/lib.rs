//! Public API contracts shared between the demo API and the CLI.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateEnvironmentRequest {
    pub project: String,
    pub region: String,
}

/// Environment lifecycle.
///
/// Happy path: `requested → provisioning → initializing → ready →
/// destroy_requested → destroying → destroyed`. `failed` and
/// `cleanup_failed` are picked up by reconciliation. Test results are tracked
/// independently of environment state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentStatus {
    Requested,
    Provisioning,
    Initializing,
    Ready,
    DestroyRequested,
    Destroying,
    Destroyed,
    Failed,
    CleanupFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Environment {
    pub id: Uuid,
    pub project: String,
    pub region: String,
    pub status: EnvironmentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiError {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_lifecycle_as_snake_case() {
        let json = serde_json::to_string(&EnvironmentStatus::DestroyRequested).unwrap();
        assert_eq!(json, r#""destroy_requested""#);
        let json = serde_json::to_string(&EnvironmentStatus::CleanupFailed).unwrap();
        assert_eq!(json, r#""cleanup_failed""#);
    }
}
