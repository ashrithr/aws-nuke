use crate::{client::*, config::TargetState, StdResult};
use colored::*;
use rusoto_core::Region;
use std::fmt;
use std::str::FromStr;
use tracing::warn;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResourceState {
    Available,
    Deleted,
    Failed,
    Pending,
    Rebooting,
    Running,
    ShuttingDown,
    Starting,
    Stopped,
    Stopping,
    Unknown,
}

impl FromStr for ResourceState {
    type Err = ();

    fn from_str(s: &str) -> StdResult<ResourceState, ()> {
        let v: &str = &s.to_lowercase();
        match v {
            "available" => Ok(ResourceState::Available),
            "pending" => Ok(ResourceState::Pending),
            "rebooting" => Ok(ResourceState::Rebooting),
            "running" | "in-use" | "associated" | "completed" | "active" | "waiting" | "ready"
            | "inservice" => Ok(ResourceState::Running),
            "shutting-down" => Ok(ResourceState::ShuttingDown),
            "starting" | "provisioning" => Ok(ResourceState::Starting),
            "stopped" => Ok(ResourceState::Stopped),
            "stopping" | "deprovisioning" => Ok(ResourceState::Stopping),
            "terminated" | "deleting" | "inactive" | "terminated_with_errors" => {
                Ok(ResourceState::Deleted)
            }
            s => {
                warn!("Failed parsing the resource-state: '{}'", s);
                Ok(ResourceState::Unknown)
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EnforcementState {
    Stop,
    Delete,
    DeleteDependent,
    Skip,
    SkipConfig,
    SkipStopped,
    SkipUnknownState,
}

impl EnforcementState {
    pub fn name(&self) -> colored::ColoredString {
        match *self {
            EnforcementState::Stop => "would be stopped".blue().bold(),
            EnforcementState::Delete => "would be removed".blue().bold(),
            EnforcementState::DeleteDependent => "would be removed (dependent)".blue().bold(),
            EnforcementState::Skip => "skipped because of rules".yellow().bold(),
            EnforcementState::SkipConfig => "skipped because of config".yellow().bold(),
            EnforcementState::SkipStopped => "skipped as resource is not running".yellow().bold(),
            EnforcementState::SkipUnknownState => {
                "skipped as resource state is unknown".yellow().bold()
            }
        }
    }

    pub fn from_target_state(target_state: &TargetState) -> Self {
        if *target_state == TargetState::Deleted {
            EnforcementState::Delete
        } else {
            EnforcementState::Stop
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EnforcementReason {
    Idle,
    Runtime,
    TagRule,
    AllowedTypeRule,
    NameRule,
    AdditionalRules,
    Dependent,
}

impl EnforcementReason {
    pub fn name(&self) -> &str {
        match *self {
            EnforcementReason::Idle => "idle",
            EnforcementReason::Runtime => "runtime",
            EnforcementReason::TagRule => "tag-not-compliant",
            EnforcementReason::AllowedTypeRule => "type-not-compliant",
            EnforcementReason::NameRule => "name-not-compliant",
            EnforcementReason::AdditionalRules => "additional-rules",
            EnforcementReason::Dependent => "dependent",
        }
    }
}

/// Logical abstraction to represent an AWS resource
#[derive(Debug, Clone)]
pub struct Resource {
    /// ID of the resource
    pub id: String,
    /// Amazon Resource Name of the resource
    pub arn: Option<String>,
    /// Type of the resource that is being generated - client mapping
    pub type_: Client,
    /// AWS Region in which the resource exists
    pub region: Region,
    /// Tags that are associated with a Resource
    pub tags: Option<Vec<NTag>>,
    /// Specifies the state of the Resource, for instance if its running,
    /// stopped, terminated, etc.
    pub state: Option<ResourceState>,
    /// Specifies the time at which the Resource is created
    pub start_time: Option<String>,
    /// Specifies the state to enforce, whether to skip it, stop it, or delete
    /// it.
    pub enforcement_state: EnforcementState,
    ///Specifies the reason for enforcement
    pub enforcement_reason: Option<EnforcementReason>,
    /// Type of the resource or the underlying resources, for instance size of
    /// the instance, db, notebook, cluster, etc.
    pub resource_type: Option<Vec<String>>,
    /// Specifies if there are any dependencies that are associated with the
    /// Resource, these dependencies will be tracked as a DAG and cleaned up
    /// in order
    pub dependencies: Option<Vec<Resource>>,
    /// Specifies if termination protection is enabled on the resource
    pub termination_protection: Option<bool>,
}

impl Default for Resource {
    fn default() -> Self {
        Resource {
            id: "root".to_string(),
            arn: None,
            type_: ClientType::DefaultClient,
            region: Region::Custom {
                name: "".to_string(),
                endpoint: "".to_string(),
            },
            tags: None,
            state: None,
            start_time: None,
            enforcement_state: EnforcementState::Skip,
            enforcement_reason: None,
            resource_type: None,
            dependencies: None,
            termination_protection: None,
        }
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}] - {} - {}",
            self.region.name().bold(),
            self.type_.name(),
            self.id.bold()
        )?;

        // if self.arn.is_some() {
        //     write!(f, " ({})", self.arn.as_ref().unwrap())?;
        // }

        if self.tags.is_some() && !self.tags.as_ref().unwrap().is_empty() {
            write!(f, " - {{")?;
            for tag in self.tags.as_ref().unwrap() {
                write!(f, "[{}]", tag)?;
            }
            write!(f, "}}")?;
        }

        write!(f, " - {}", self.enforcement_state.name())?;

        if self.enforcement_reason.is_some() {
            write!(f, " ({})", self.enforcement_reason.as_ref().unwrap().name())?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct NTag {
    pub key: Option<String>,
    pub value: Option<String>,
}

impl fmt::Display for NTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.key.is_some() && self.value.is_some() {
            write!(
                f,
                "{} -> {}",
                self.key.as_ref().unwrap().on_white().black(),
                self.value.as_ref().unwrap().on_white().black()
            )
        } else {
            write!(f, "{:?} -> {:?}", self.key, self.value)
        }
    }
}
