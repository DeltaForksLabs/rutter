// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::num::NonZeroU32;
use std::rc::Rc;

use skia_safe::{Canvas, Surface as SkiaSurface};
use softbuffer::{Context, Surface};
use winit::{dpi::PhysicalSize, window::Window};

use super::{BackendFailure, BackendType, GraphicsBackend, GraphicsError};

const MAX_FRAME_BYTES: u64 = 512 * 1024 * 1024;

pub(super) fn validate_cpu_surface_transparency(transparent: bool) -> Result<(), BackendFailure> {
    if !transparent {
        return Ok(());
    }
    Err(BackendFailure::new(
        BackendType::CpuSoftbuffer,
        "transparent surfaces require alpha presentation; softbuffer currently presents RGB only",
    ))
}

fn validate_frame_size(size: PhysicalSize<u32>) -> Result<(i32, i32), GraphicsError> {
    let pixels = u64::from(size.width)
        .checked_mul(u64::from(size.height))
        .ok_or(GraphicsError::InvalidSurfaceSize {
            width: size.width,
            height: size.height,
        })?;
    let bytes = pixels
        .checked_mul(4)
        .ok_or(GraphicsError::InvalidSurfaceSize {
            width: size.width,
            height: size.height,
        })?;
    if bytes > MAX_FRAME_BYTES {
        return Err(GraphicsError::SurfaceTooLarge {
            width: size.width,
            height: size.height,
            bytes,
            max_bytes: MAX_FRAME_BYTES,
        });
    }
    let width = i32::try_from(size.width).map_err(|_| GraphicsError::InvalidSurfaceSize {
        width: size.width,
        height: size.height,
    })?;
    let height = i32::try_from(size.height).map_err(|_| GraphicsError::InvalidSurfaceSize {
        width: size.width,
        height: size.height,
    })?;
    Ok((width.max(1), height.max(1)))
}

pub struct CpuBackend {
    window: Rc<Window>,
    _context: Context<Rc<Window>>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    skia_surface: SkiaSurface,
}

impl CpuBackend {
    pub fn new(window: Rc<Window>) -> Result<Self, GraphicsError> {
        let context = Context::new(window.clone()).map_err(|err| {
            GraphicsError::BackendInit(BackendFailure::new(
                BackendType::CpuSoftbuffer,
                err.to_string(),
            ))
        })?;
        let mut surface = Surface::new(&context, window.clone()).map_err(|err| {
            GraphicsError::BackendInit(BackendFailure::new(
                BackendType::CpuSoftbuffer,
                err.to_string(),
            ))
        })?;
        let size = window.inner_size();
        let raster_size = validate_frame_size(size)?;
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            surface.resize(width, height).map_err(|err| {
                GraphicsError::BackendInit(BackendFailure::new(
                    BackendType::CpuSoftbuffer,
                    err.to_string(),
                ))
            })?;
        }
        let skia_surface = skia_safe::surfaces::raster_n32_premul((raster_size.0, raster_size.1))
            .ok_or_else(|| {
            GraphicsError::BackendInit(BackendFailure::new(
                BackendType::CpuSoftbuffer,
                "failed to create raster surface",
            ))
        })?;

        Ok(Self {
            window,
            _context: context,
            surface,
            skia_surface,
        })
    }
}

impl GraphicsBackend for CpuBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::CpuSoftbuffer
    }

    fn begin_frame(&mut self) -> Result<&Canvas, GraphicsError> {
        Ok(self.skia_surface.canvas())
    }

    fn end_frame(&mut self) -> Result<(), GraphicsError> {
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|err| GraphicsError::Frame {
                backend: BackendType::CpuSoftbuffer,
                reason: err.to_string(),
            })?;
        let pixels = self
            .skia_surface
            .peek_pixels()
            .ok_or_else(|| GraphicsError::Frame {
                backend: BackendType::CpuSoftbuffer,
                reason: "failed to peek raster pixels".to_string(),
            })?;
        let raw = pixels.bytes().ok_or_else(|| GraphicsError::Frame {
            backend: BackendType::CpuSoftbuffer,
            reason: "failed to access raster bytes".to_string(),
        })?;

        for (index, pixel) in buffer.iter_mut().enumerate() {
            let offset = index * 4;
            if offset + 2 < raw.len() {
                *pixel = ((raw[offset] as u32) << 16)
                    | ((raw[offset + 1] as u32) << 8)
                    | raw[offset + 2] as u32;
            }
        }

        buffer.present().map_err(|err| GraphicsError::Frame {
            backend: BackendType::CpuSoftbuffer,
            reason: err.to_string(),
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) -> Result<(), GraphicsError> {
        let raster_size = validate_frame_size(size)?;
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            self.surface
                .resize(width, height)
                .map_err(|err| GraphicsError::Resize {
                    backend: BackendType::CpuSoftbuffer,
                    reason: err.to_string(),
                })?;
        }

        self.skia_surface = skia_safe::surfaces::raster_n32_premul((raster_size.0, raster_size.1))
            .ok_or_else(|| GraphicsError::Resize {
                backend: BackendType::CpuSoftbuffer,
                reason: "failed to recreate raster surface".to_string(),
            })?;

        Ok(())
    }

    fn window(&self) -> &Rc<Window> {
        &self.window
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GraphicsError, MAX_FRAME_BYTES, validate_cpu_surface_transparency, validate_frame_size,
    };
    use winit::dpi::PhysicalSize;

    #[test]
    fn validate_frame_size_rejects_unrepresentable_and_oversized_dimensions() {
        assert_eq!(
            validate_frame_size(PhysicalSize::new(0, 0)).unwrap(),
            (1, 1)
        );
        assert!(matches!(
            validate_frame_size(PhysicalSize::new(u32::MAX, 1)),
            Err(GraphicsError::SurfaceTooLarge { .. })
        ));
        assert!(matches!(
            validate_frame_size(PhysicalSize::new(u32::MAX, u32::MAX)),
            Err(GraphicsError::InvalidSurfaceSize { .. })
        ));
        assert!(matches!(
            validate_frame_size(PhysicalSize::new(16_385, 8_192)),
            Err(GraphicsError::SurfaceTooLarge {
                max_bytes: MAX_FRAME_BYTES,
                ..
            })
        ));
    }

    #[test]
    fn transparent_surface_is_rejected_by_cpu_backend() {
        assert!(validate_cpu_surface_transparency(false).is_ok());
        let failure = validate_cpu_surface_transparency(true).unwrap_err();
        assert_eq!(failure.backend, super::BackendType::CpuSoftbuffer);
    }
}
