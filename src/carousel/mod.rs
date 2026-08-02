// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

mod config;
pub(crate) mod geometry;
mod position;
mod state;

pub use config::{CarouselConfig, CarouselConfigError};
pub use state::CarouselState;

pub(crate) use config::CarouselSizing;
pub(crate) use position::CarouselPosition;
