// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

pub mod cpu;
pub mod gl;
pub mod vk;

use std::fmt;
use std::rc::Rc;

use skia_safe::{Canvas, gpu::DirectContext};
use winit::{
    dpi::PhysicalSize,
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

use self::cpu::CpuBackend;
use self::gl::GlBackend;
use self::vk::VkBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Vulkan,
    OpenGl,
    CpuSoftbuffer,
}

impl fmt::Display for BackendType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Vulkan => "vulkan",
            Self::OpenGl => "opengl",
            Self::CpuSoftbuffer => "softbuffer",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone)]
pub struct BackendFailure {
    pub backend: BackendType,
    pub reason: String,
}

impl BackendFailure {
    pub fn new(backend: BackendType, reason: impl Into<String>) -> Self {
        Self {
            backend,
            reason: reason.into(),
        }
    }
}

#[derive(Debug)]
pub enum GraphicsError {
    BackendInit(BackendFailure),
    Resize {
        backend: BackendType,
        reason: String,
    },
    Frame {
        backend: BackendType,
        reason: String,
    },
    BackendUnavailable {
        operation: &'static str,
    },
    NoBackendAvailable(Vec<BackendFailure>),
}

impl fmt::Display for GraphicsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendInit(failure) => {
                write!(
                    f,
                    "failed to initialize {} backend: {}",
                    failure.backend, failure.reason
                )
            }
            Self::Resize { backend, reason } => {
                write!(f, "failed to resize {} backend: {}", backend, reason)
            }
            Self::Frame { backend, reason } => {
                write!(
                    f,
                    "failed to present frame on {} backend: {}",
                    backend, reason
                )
            }
            Self::BackendUnavailable { operation } => {
                write!(f, "graphics backend unavailable during {operation}")
            }
            Self::NoBackendAvailable(failures) => {
                write!(f, "no graphics backend could be initialized")?;
                for failure in failures {
                    write!(f, " [{}: {}]", failure.backend, failure.reason)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for GraphicsError {}

pub trait GraphicsBackend {
    fn backend_type(&self) -> BackendType;
    fn begin_frame(&mut self) -> Result<&Canvas, GraphicsError>;
    fn end_frame(&mut self) -> Result<(), GraphicsError>;
    fn resize(&mut self, size: PhysicalSize<u32>) -> Result<(), GraphicsError>;
    fn window(&self) -> &Rc<Window>;
    fn skia_context(&mut self) -> Option<&mut DirectContext> {
        None
    }
}

pub fn create_best_backend(
    event_loop: &ActiveEventLoop,
    attrs: WindowAttributes,
) -> Result<Box<dyn GraphicsBackend>, GraphicsError> {
    let mut failures = Vec::new();

    match VkBackend::try_new(event_loop, attrs.clone()) {
        Ok(backend) => return Ok(backend),
        Err(err) => failures.push(err),
    }

    match GlBackend::try_new(event_loop, attrs.clone()) {
        Ok(backend) => return Ok(backend),
        Err(err) => failures.push(err),
    }

    let window = Rc::new(event_loop.create_window(attrs).map_err(|err| {
        GraphicsError::BackendInit(BackendFailure::new(
            BackendType::CpuSoftbuffer,
            err.to_string(),
        ))
    })?);

    match CpuBackend::new(window) {
        Ok(backend) => Ok(Box::new(backend)),
        Err(err) => {
            if let GraphicsError::BackendInit(failure) = err {
                failures.push(failure);
            }
            Err(GraphicsError::NoBackendAvailable(failures))
        }
    }
}
