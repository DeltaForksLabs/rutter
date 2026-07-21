// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use winit::error::EventLoopError;

use crate::widget_id::WidgetIdError;

/// Describes a controlled failure while starting or running a Rutter application.
#[derive(Debug)]
pub enum RutterRunError {
    EventLoop(EventLoopError),
    WidgetId(WidgetIdError),
}

impl Display for RutterRunError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLoop(error) => write!(formatter, "event loop failed: {error}"),
            Self::WidgetId(error) => write!(formatter, "widget ID validation failed: {error}"),
        }
    }
}

impl Error for RutterRunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::EventLoop(error) => Some(error),
            Self::WidgetId(error) => Some(error),
        }
    }
}

impl From<EventLoopError> for RutterRunError {
    fn from(error: EventLoopError) -> Self {
        Self::EventLoop(error)
    }
}

impl From<WidgetIdError> for RutterRunError {
    fn from(error: WidgetIdError) -> Self {
        Self::WidgetId(error)
    }
}
