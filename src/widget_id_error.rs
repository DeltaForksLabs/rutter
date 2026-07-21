// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

const MANUAL_ID_EXPECTATION: &str = "a non-zero manual ID with bit 63 clear";
const UNIQUE_ID_EXPECTATION: &str = "each resolved ID to have exactly one owner";
const TRANSITION_EXPECTATION: &str =
    "the same logical family and, for automatic IDs, the same tree path";
const RECONSTRUCTION_EXPECTATION: &str =
    "both reconstructions to contain identical resolved ID owners";

/// Reports invalid raw IDs and ownership conflicts found while validating widget trees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetIdError {
    ReservedValue {
        value: u64,
    },
    Duplicate {
        value: u64,
        first_type: &'static str,
        first_path: Vec<usize>,
        second_type: &'static str,
        second_path: Vec<usize>,
    },
    IncompatibleReuse {
        value: u64,
        previous_type: &'static str,
        previous_path: Vec<usize>,
        next_type: &'static str,
        next_path: Vec<usize>,
    },
    InconsistentTree {
        value: u64,
        validated_owner: Option<String>,
        rebuilt_owner: Option<String>,
    },
    RuntimeOverride {
        value: u64,
        cache: &'static str,
    },
    UnsupportedWidget {
        value: u64,
    },
    UnexpectedOwner {
        value: u64,
        actual_type: Option<&'static str>,
        expected_type: &'static str,
    },
    InconsistentStructure {
        index: usize,
        validated_type: Option<&'static str>,
        rebuilt_type: Option<&'static str>,
    },
}

impl Display for WidgetIdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedValue { value } => write!(
                formatter,
                "widget ID value {value} is reserved; expected {MANUAL_ID_EXPECTATION}"
            ),
            Self::Duplicate {
                value,
                first_type,
                first_path,
                second_type,
                second_path,
            } => write!(
                formatter,
                "widget ID value {value} is owned by {first_type} at {} and {second_type} at {}; expected {UNIQUE_ID_EXPECTATION}",
                TreePath(first_path),
                TreePath(second_path)
            ),
            Self::IncompatibleReuse {
                value,
                previous_type,
                previous_path,
                next_type,
                next_path,
            } => write!(
                formatter,
                "widget ID value {value} changed from {previous_type} at {} to {next_type} at {}; expected {TRANSITION_EXPECTATION}",
                TreePath(previous_path),
                TreePath(next_path)
            ),
            Self::InconsistentTree {
                value,
                validated_owner,
                rebuilt_owner,
            } => write!(
                formatter,
                "widget ID value {value} has validated owner {} and rebuilt owner {}; expected {RECONSTRUCTION_EXPECTATION}",
                validated_owner.as_deref().unwrap_or("<missing>"),
                rebuilt_owner.as_deref().unwrap_or("<missing>")
            ),
            Self::RuntimeOverride { value, cache } => write!(
                formatter,
                "widget ID value {value} already exists in runtime cache {cache}; expected each validated owner to be inserted exactly once"
            ),
            Self::UnsupportedWidget { value } => write!(
                formatter,
                "widget ID value {value} cannot be assigned to this widget; expected a widget variant with an explicit ID field"
            ),
            Self::UnexpectedOwner {
                value,
                actual_type,
                expected_type,
            } => write!(
                formatter,
                "widget ID value {value} belongs to {}; expected owner type {expected_type}",
                actual_type.unwrap_or("<missing>")
            ),
            Self::InconsistentStructure {
                index,
                validated_type,
                rebuilt_type,
            } => write!(
                formatter,
                "widget tree structure differs at node {index}: validated {} and rebuilt {}; expected identical widget variants for each reconstruction",
                validated_type.unwrap_or("<missing>"),
                rebuilt_type.unwrap_or("<missing>")
            ),
        }
    }
}

impl Error for WidgetIdError {}

struct TreePath<'a>(&'a [usize]);

impl Display for TreePath<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("root")?;
        for segment in self.0 {
            write!(formatter, "/{segment}")?;
        }
        Ok(())
    }
}
