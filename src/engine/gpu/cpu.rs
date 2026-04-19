use std::num::NonZeroU32;
use std::rc::Rc;

use skia_safe::{Canvas, Surface as SkiaSurface};
use softbuffer::{Context, Surface};
use winit::{dpi::PhysicalSize, window::Window};

use super::{BackendFailure, BackendType, GraphicsBackend, GraphicsError};

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
        let skia_surface = skia_safe::surfaces::raster_n32_premul((
            size.width.max(1) as i32,
            size.height.max(1) as i32,
        ))
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

        self.skia_surface = skia_safe::surfaces::raster_n32_premul((
            size.width.max(1) as i32,
            size.height.max(1) as i32,
        ))
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
