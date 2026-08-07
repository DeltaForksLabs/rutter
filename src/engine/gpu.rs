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

use self::cpu::{CpuBackend, validate_cpu_surface_transparency};
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
    InvalidSurfaceSize {
        width: u32,
        height: u32,
    },
    SurfaceTooLarge {
        width: u32,
        height: u32,
        bytes: u64,
        max_bytes: u64,
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
            Self::InvalidSurfaceSize { width, height } => {
                write!(
                    f,
                    "invalid surface size `{width}x{height}`, expected dimensions representable as positive i32 values"
                )
            }
            Self::SurfaceTooLarge {
                width,
                height,
                bytes,
                max_bytes,
            } => {
                write!(
                    f,
                    "surface size `{width}x{height}` requires {bytes} bytes, expected <= {max_bytes} bytes"
                )
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
    #[deprecated(
        note = "borrowed Skia contexts cannot preserve backend activation across multiple surfaces"
    )]
    fn skia_context(&mut self) -> Option<&mut DirectContext> {
        None
    }
}

pub fn create_best_backend(
    event_loop: &ActiveEventLoop,
    attrs: WindowAttributes,
) -> Result<Box<dyn GraphicsBackend>, GraphicsError> {
    let mut failures = Vec::new();
    let transparent = attrs.transparent();

    if let Some(backend) =
        accept_backend_candidate(&mut failures, VkBackend::try_new(event_loop, attrs.clone()))
    {
        return Ok(backend);
    }

    if let Some(backend) =
        accept_backend_candidate(&mut failures, GlBackend::try_new(event_loop, attrs.clone()))
    {
        return Ok(backend);
    }

    if let Err(failure) = validate_cpu_surface_transparency(transparent) {
        failures.push(failure);
        return Err(GraphicsError::NoBackendAvailable(failures));
    }
    let window = match event_loop.create_window(attrs) {
        Ok(window) => Rc::new(window),
        Err(error) => {
            failures.push(BackendFailure::new(
                BackendType::CpuSoftbuffer,
                error.to_string(),
            ));
            return Err(GraphicsError::NoBackendAvailable(failures));
        }
    };

    match CpuBackend::new(window) {
        Ok(backend) => Ok(Box::new(backend)),
        Err(error) => {
            failures.push(cpu_backend_failure(error));
            Err(GraphicsError::NoBackendAvailable(failures))
        }
    }
}

fn cpu_backend_failure(error: GraphicsError) -> BackendFailure {
    match error {
        GraphicsError::BackendInit(failure) => failure,
        error => BackendFailure::new(BackendType::CpuSoftbuffer, error.to_string()),
    }
}

fn accept_backend_candidate<Backend>(
    failures: &mut Vec<BackendFailure>,
    candidate: Result<Backend, BackendFailure>,
) -> Option<Backend> {
    match candidate {
        Ok(backend) => Some(backend),
        Err(failure) => {
            failures.push(failure);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BackendFailure, BackendType, GraphicsError, accept_backend_candidate, cpu_backend_failure,
    };

    #[test]
    fn vulkan_probe_failure_allows_opengl_candidate() {
        let mut failures = Vec::new();
        let vulkan = Err(BackendFailure::new(BackendType::Vulkan, "probe failed"));
        let opengl = Ok("OpenGL surface");

        assert_eq!(
            accept_backend_candidate(&mut failures, vulkan),
            None::<&str>
        );
        assert_eq!(
            accept_backend_candidate(&mut failures, opengl),
            Some("OpenGL surface")
        );
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].backend, BackendType::Vulkan);
    }

    #[test]
    fn cpu_surface_failure_is_preserved_in_fallback_diagnostics() {
        let failure = cpu_backend_failure(GraphicsError::InvalidSurfaceSize {
            width: u32::MAX,
            height: 480,
        });

        assert_eq!(failure.backend, BackendType::CpuSoftbuffer);
        assert!(failure.reason.contains(&u32::MAX.to_string()));
        assert!(failure.reason.contains("expected dimensions representable"));
    }
}
