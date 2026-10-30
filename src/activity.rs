// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Renderer-independent, owned Dynamic Island activity snapshots.
//!
//! Create a [`Transfer`] or [`Media`], register an owner, then start an activity.
//! Replacement is a full snapshot, including its kind; no partial updates or
//! implicit playback/progress advancement occur. Strings are bounded in UTF-8
//! **bytes**, never truncated, and may be empty. They are plain display text,
//! not markup, paths, artwork, commands, or executable instructions.
//!
//! Registry clones are independent snapshots, suitable for GUI state. IDs are
//! allocated monotonically per kind across the host process, including across
//! clones and independent registries; removal never permits reuse. Only IDs
//! present in a particular registry can be used there. An ID is an identifier,
//! not authentication: transports must bind callers to their owner.

#![doc = include_str!("../docs/ACTIVITIES.md")]

mod model;
mod registry;

pub mod view;

#[cfg(all(unix, feature = "activity-ipc"))]
pub mod ipc;

pub use model::{
    ActivityContent, ActivitySummary, Media, MediaState, Transfer, TransferDirection, TransferState,
};
pub use registry::{Activity, ActivityRegistry};

use std::fmt;

/// Maximum title length in UTF-8 bytes.
pub const MAX_TITLE_BYTES: usize = 256;
/// Maximum supporting text (including media artist) length in UTF-8 bytes.
pub const MAX_SUPPORTING_BYTES: usize = 2048;
/// Maximum simultaneously registered owners.
pub const MAX_OWNERS: usize = 16;
/// Maximum live activities across all owners.
pub const MAX_ACTIVITIES: usize = 64;
/// Maximum live activities belonging to one owner.
pub const MAX_ACTIVITIES_PER_OWNER: usize = 8;

/// Opaque host-allocated owner identity. No public integer constructor exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnerId(u64);

/// Opaque host-allocated activity identity. No public integer constructor exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActivityId(u64);

/// A semantic request; routing does not mutate the current snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityAction {
    Pause,
    Resume,
    Cancel,
    Previous,
    Next,
}

/// A validated request to deliver to the owning producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivityActionEvent {
    owner: OwnerId,
    id: ActivityId,
    action: ActivityAction,
}

impl ActivityActionEvent {
    pub fn owner(&self) -> OwnerId {
        self.owner
    }
    pub fn id(&self) -> ActivityId {
        self.id
    }
    pub fn action(&self) -> ActivityAction {
        self.action
    }
}

/// The resource whose hard limit was reached; nothing is evicted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapacityKind {
    Owners,
    Activities,
    OwnerActivities(OwnerId),
}

/// Host ID sequence that could not advance without integer overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdKind {
    Owner,
    Activity,
}

/// Invalid snapshot fields. Numeric values are retained; text is never echoed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidField {
    TextTooLong {
        field: &'static str,
        bytes: usize,
        max: usize,
    },
    ProgressExceedsTotal {
        completed: u64,
        total: u64,
    },
    IncompleteTransfer {
        completed: u64,
        total: u64,
    },
    PositionExceedsDuration {
        position: std::time::Duration,
        duration: std::time::Duration,
    },
}

/// Contextual, recoverable failures. Failed operations leave the registry intact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityError {
    InvalidField(InvalidField),
    UnknownOwner(OwnerId),
    UnknownActivity(ActivityId),
    Ownership {
        owner: OwnerId,
        id: ActivityId,
        actual_owner: OwnerId,
    },
    Capacity {
        kind: CapacityKind,
        limit: usize,
    },
    IdExhausted(IdKind),
    UnsupportedAction {
        id: ActivityId,
        action: ActivityAction,
    },
}

impl fmt::Display for ActivityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidField(field) => write!(f, "invalid activity snapshot field: {field:?}"),
            Self::UnknownOwner(owner) => {
                write!(f, "activity operation requires registered owner {owner:?}")
            }
            Self::UnknownActivity(id) => {
                write!(f, "activity operation requires live activity {id:?}")
            }
            Self::Ownership {
                owner,
                id,
                actual_owner,
            } => write!(
                f,
                "owner {owner:?} cannot modify activity {id:?} belonging to {actual_owner:?}"
            ),
            Self::Capacity { kind, limit } => write!(
                f,
                "cannot allocate activity resource: {kind:?} limit {limit} reached"
            ),
            Self::IdExhausted(kind) => write!(
                f,
                "cannot allocate {kind:?} ID: host integer sequence exhausted"
            ),
            Self::UnsupportedAction { id, action } => write!(
                f,
                "activity {id:?} does not support {action:?} in its current snapshot"
            ),
        }
    }
}

impl std::error::Error for ActivityError {}

#[cfg(test)]
mod tests;
