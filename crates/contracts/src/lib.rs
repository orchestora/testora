//! Public API contracts shared between the API and the CLI.
use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Longest accepted project name.
pub const PROJECT_MAX_LEN: usize = 64;
/// Longest accepted region label.
pub const REGION_MAX_LEN: usize = 32;

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

impl CreateEnvironmentRequest {
    /// Same rules on client and server, so the CLI fails fast with the
    /// message the API would return.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_project(&self.project)?;
        validate_region(&self.region)
    }
}

/// A request field broke a validation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}

/// Project names: 1–64 ASCII letters, digits, `-` or `_`, starting with a
/// letter or digit.
pub fn validate_project(value: &str) -> Result<(), ValidationError> {
    let err = |message: String| {
        Err(ValidationError {
            field: "project",
            message,
        })
    };
    if value.is_empty() {
        return err("must not be empty".into());
    }
    if value.len() > PROJECT_MAX_LEN {
        return err(format!("must be at most {PROJECT_MAX_LEN} characters"));
    }
    let mut bytes = value.bytes();
    let first_ok = bytes.next().is_some_and(|b| b.is_ascii_alphanumeric());
    if !first_ok || !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return err(
            "may contain only ASCII letters, digits, '-' and '_', and must start with a letter or digit"
                .into(),
        );
    }
    Ok(())
}

/// Region labels: 1–32 lowercase letters, digits or `-`, not starting or
/// ending with `-` (e.g. `us-east`).
pub fn validate_region(value: &str) -> Result<(), ValidationError> {
    let err = |message: String| {
        Err(ValidationError {
            field: "region",
            message,
        })
    };
    if value.is_empty() {
        return err("must not be empty".into());
    }
    if value.len() > REGION_MAX_LEN {
        return err(format!("must be at most {REGION_MAX_LEN} characters"));
    }
    let valid = !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !valid {
        return err(
            "must be lowercase letters, digits and '-', not starting or ending with '-'".into(),
        );
    }
    Ok(())
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

/// Error body for every non-2xx API response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiError {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_STATUSES: [(EnvironmentStatus, &str); 9] = [
        (EnvironmentStatus::Requested, "requested"),
        (EnvironmentStatus::Provisioning, "provisioning"),
        (EnvironmentStatus::Initializing, "initializing"),
        (EnvironmentStatus::Ready, "ready"),
        (EnvironmentStatus::DestroyRequested, "destroy_requested"),
        (EnvironmentStatus::Destroying, "destroying"),
        (EnvironmentStatus::Destroyed, "destroyed"),
        (EnvironmentStatus::Failed, "failed"),
        (EnvironmentStatus::CleanupFailed, "cleanup_failed"),
    ];

    #[test]
    fn every_status_round_trips_as_snake_case() {
        for (status, wire) in ALL_STATUSES {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{wire}\""));
            let back: EnvironmentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status);
        }
    }

    #[test]
    fn unknown_status_is_rejected() {
        assert!(serde_json::from_str::<EnvironmentStatus>(r#""paused""#).is_err());
        assert!(serde_json::from_str::<EnvironmentStatus>(r#""Ready""#).is_err());
    }

    #[test]
    fn environment_round_trips() {
        let env = Environment {
            id: Uuid::new_v4(),
            project: "demo".into(),
            region: "us-east".into(),
            status: EnvironmentStatus::Ready,
        };
        let back: Environment =
            serde_json::from_str(&serde_json::to_string(&env).unwrap()).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn project_accepts_valid_names() {
        for ok in [
            "demo",
            "Demo_App-2",
            "a",
            "9lives",
            &"p".repeat(PROJECT_MAX_LEN),
        ] {
            assert_eq!(validate_project(ok), Ok(()), "{ok:?} should be valid");
        }
    }

    #[test]
    fn project_rejects_invalid_names() {
        let too_long = "p".repeat(PROJECT_MAX_LEN + 1);
        for bad in [
            "",
            " ",
            " demo",
            "demo app",
            "-demo",
            "_demo",
            "demo/../x",
            "démo",
            "demo\n",
            too_long.as_str(),
        ] {
            let err = validate_project(bad).expect_err(&format!("{bad:?} should be invalid"));
            assert_eq!(err.field, "project");
        }
    }

    #[test]
    fn region_accepts_valid_labels() {
        for ok in [
            "us-east",
            "local",
            "ap-southeast-1",
            "x",
            &"r".repeat(REGION_MAX_LEN),
        ] {
            assert_eq!(validate_region(ok), Ok(()), "{ok:?} should be valid");
        }
    }

    #[test]
    fn region_rejects_invalid_labels() {
        let too_long = "r".repeat(REGION_MAX_LEN + 1);
        for bad in [
            "",
            "US-EAST",
            "us_east",
            "us east",
            "-us",
            "us-",
            "us.east",
            too_long.as_str(),
        ] {
            let err = validate_region(bad).expect_err(&format!("{bad:?} should be invalid"));
            assert_eq!(err.field, "region");
        }
    }

    #[test]
    fn request_validation_reports_first_bad_field() {
        let req = CreateEnvironmentRequest {
            project: String::new(),
            region: "BAD".into(),
        };
        assert_eq!(req.validate().unwrap_err().field, "project");

        let req = CreateEnvironmentRequest {
            project: "demo".into(),
            region: "BAD".into(),
        };
        let err = req.validate().unwrap_err();
        assert_eq!(err.field, "region");
        assert!(err.to_string().starts_with("region: "));
    }
}
