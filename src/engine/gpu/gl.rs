// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::ffi::CString;
use std::num::NonZeroU32;
use std::ptr;
use std::rc::Rc;

use gl::types::GLint;
use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::{GlSurface, NotCurrentGlContext};
use glutin::surface::{
    Surface as GlutinSurface, SurfaceAttributesBuilder, SwapInterval, WindowSurface,
};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use skia_safe::{
    Canvas, ColorType, Surface as SkiaSurface,
    gpu::{
        self, DirectContext, SurfaceOrigin, backend_render_targets, direct_contexts,
        gl::FramebufferInfo,
    },
};
use winit::{
    dpi::PhysicalSize,
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

use super::{BackendFailure, BackendType, GraphicsBackend, GraphicsError};

pub struct GlBackend {
    window: Rc<Window>,
    gl_context: PossiblyCurrentContext,
    gl_surface: GlutinSurface<WindowSurface>,
    skia_context: DirectContext,
    skia_surface: SkiaSurface,
    fb_info: FramebufferInfo,
    sample_count: usize,
    stencil_size: usize,
}

impl GlBackend {
    pub fn try_new(
        event_loop: &ActiveEventLoop,
        attrs: WindowAttributes,
    ) -> Result<Box<dyn GraphicsBackend>, BackendFailure> {
        let transparent = attrs.transparent();
        // A broad template lets the picker return a typed failure when no alpha-capable
        // config exists instead of letting glutin's infallible picker receive an empty set.
        let template = ConfigTemplateBuilder::new();
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(attrs));
        let (window, gl_config) = display_builder
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|accum, config| {
                        if prefer_gl_surface_config(
                            transparent,
                            config.supports_transparency(),
                            config.num_samples(),
                            accum.supports_transparency(),
                            accum.num_samples(),
                        ) {
                            config
                        } else {
                            accum
                        }
                    })
                    .expect("glutin returned no GL configs")
            })
            .map_err(|err| BackendFailure::new(BackendType::OpenGl, err.to_string()))?;
        validate_gl_surface_transparency(
            transparent,
            gl_config.supports_transparency(),
            gl_config.alpha_size(),
        )
        .map_err(|reason| BackendFailure::new(BackendType::OpenGl, reason))?;

        let window = Rc::new(window.ok_or_else(|| {
            BackendFailure::new(
                BackendType::OpenGl,
                "glutin did not create a compatible window",
            )
        })?);

        let window_handle = window
            .window_handle()
            .map_err(|err| BackendFailure::new(BackendType::OpenGl, err.to_string()))?;
        let raw_window_handle = window_handle.as_raw();

        let context_attributes = ContextAttributesBuilder::new().build(Some(raw_window_handle));
        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(Some(Version::new(3, 0))))
            .build(Some(raw_window_handle));

        let display = gl_config.display();
        let not_current = unsafe {
            display
                .create_context(&gl_config, &context_attributes)
                .or_else(|_| display.create_context(&gl_config, &fallback_context_attributes))
        }
        .map_err(|err| BackendFailure::new(BackendType::OpenGl, err.to_string()))?;

        let size = window.inner_size();
        let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window_handle,
            NonZeroU32::new(size.width.max(1)).unwrap(),
            NonZeroU32::new(size.height.max(1)).unwrap(),
        );
        let gl_surface = unsafe { display.create_window_surface(&gl_config, &surface_attributes) }
            .map_err(|err| BackendFailure::new(BackendType::OpenGl, err.to_string()))?;
        let gl_context = not_current
            .make_current(&gl_surface)
            .map_err(|err| BackendFailure::new(BackendType::OpenGl, err.to_string()))?;

        let _ = gl_surface
            .set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()));

        gl::load_with(|symbol| {
            display.get_proc_address(CString::new(symbol).unwrap().as_c_str()) as *const _
        });

        let interface = gpu::gl::Interface::new_load_with(|name| {
            if name == "eglGetCurrentDisplay" {
                return ptr::null();
            }
            display.get_proc_address(CString::new(name).unwrap().as_c_str())
        })
        .ok_or_else(|| {
            BackendFailure::new(BackendType::OpenGl, "failed to create Skia GL interface")
        })?;

        let mut skia_context = direct_contexts::make_gl(interface, None).ok_or_else(|| {
            BackendFailure::new(
                BackendType::OpenGl,
                "failed to create Skia direct GL context",
            )
        })?;

        let fb_info = current_framebuffer_info();
        let sample_count = gl_config.num_samples() as usize;
        let stencil_size = gl_config.stencil_size() as usize;
        let skia_surface = create_skia_surface(
            &window,
            &mut skia_context,
            fb_info,
            sample_count,
            stencil_size,
        )
        .map_err(|reason| BackendFailure::new(BackendType::OpenGl, reason))?;

        Ok(Box::new(Self {
            window,
            gl_context,
            gl_surface,
            skia_context,
            skia_surface,
            fb_info,
            sample_count,
            stencil_size,
        }))
    }
}

fn prefer_gl_surface_config(
    transparent: bool,
    candidate_transparency: Option<bool>,
    candidate_samples: u8,
    current_transparency: Option<bool>,
    current_samples: u8,
) -> bool {
    let candidate_alpha = candidate_transparency == Some(true);
    let current_alpha = current_transparency == Some(true);
    if transparent && candidate_alpha != current_alpha {
        return candidate_alpha;
    }
    candidate_samples < current_samples
}

fn validate_gl_surface_transparency(
    transparent: bool,
    reported_support: Option<bool>,
    alpha_size: u8,
) -> Result<(), String> {
    if !transparent || reported_support == Some(true) && alpha_size > 0 {
        return Ok(());
    }
    Err(format!(
        "OpenGL transparency support is {reported_support:?} with {alpha_size} alpha bits; expected confirmed transparency and alpha_size > 0"
    ))
}

fn current_framebuffer_info() -> FramebufferInfo {
    let mut framebuffer_id: GLint = 0;
    unsafe {
        gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut framebuffer_id);
    }

    FramebufferInfo {
        fboid: framebuffer_id as u32,
        format: gpu::gl::Format::RGBA8.into(),
        ..Default::default()
    }
}

fn create_skia_surface(
    window: &Window,
    skia_context: &mut DirectContext,
    fb_info: FramebufferInfo,
    sample_count: usize,
    stencil_size: usize,
) -> Result<SkiaSurface, String> {
    let size = window.inner_size();
    let render_target = backend_render_targets::make_gl(
        (
            size.width
                .try_into()
                .map_err(|_| "width overflow".to_string())?,
            size.height
                .try_into()
                .map_err(|_| "height overflow".to_string())?,
        ),
        sample_count,
        stencil_size,
        fb_info,
    );

    gpu::surfaces::wrap_backend_render_target(
        skia_context,
        &render_target,
        SurfaceOrigin::BottomLeft,
        ColorType::RGBA8888,
        None,
        None,
    )
    .ok_or_else(|| "failed to wrap OpenGL framebuffer with Skia surface".to_string())
}

impl GraphicsBackend for GlBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::OpenGl
    }

    fn begin_frame(&mut self) -> Result<&Canvas, GraphicsError> {
        Ok(self.skia_surface.canvas())
    }

    fn end_frame(&mut self) -> Result<(), GraphicsError> {
        self.skia_context
            .flush_and_submit_surface(&mut self.skia_surface, None);
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .map_err(|err| GraphicsError::Frame {
                backend: BackendType::OpenGl,
                reason: err.to_string(),
            })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) -> Result<(), GraphicsError> {
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        self.gl_surface.resize(
            &self.gl_context,
            NonZeroU32::new(size.width.max(1)).unwrap(),
            NonZeroU32::new(size.height.max(1)).unwrap(),
        );
        self.fb_info = current_framebuffer_info();
        self.skia_surface = create_skia_surface(
            &self.window,
            &mut self.skia_context,
            self.fb_info,
            self.sample_count,
            self.stencil_size,
        )
        .map_err(|reason| GraphicsError::Resize {
            backend: BackendType::OpenGl,
            reason,
        })?;
        Ok(())
    }

    fn window(&self) -> &Rc<Window> {
        &self.window
    }

    fn skia_context(&mut self) -> Option<&mut DirectContext> {
        Some(&mut self.skia_context)
    }
}

#[cfg(test)]
mod tests {
    use super::{prefer_gl_surface_config, validate_gl_surface_transparency};

    #[test]
    fn transparent_gl_surface_requires_confirmed_alpha_support() {
        assert!(validate_gl_surface_transparency(true, Some(true), 8).is_ok());
        assert!(validate_gl_surface_transparency(true, Some(false), 8).is_err());
        assert!(validate_gl_surface_transparency(true, None, 8).is_err());
        assert!(validate_gl_surface_transparency(true, Some(true), 0).is_err());
        assert!(validate_gl_surface_transparency(false, None, 0).is_ok());
    }

    #[test]
    fn transparent_gl_config_outranks_lower_sample_opaque_config() {
        assert!(!prefer_gl_surface_config(
            true,
            Some(false),
            0,
            Some(true),
            8
        ));
        assert!(prefer_gl_surface_config(
            true,
            Some(true),
            8,
            Some(false),
            0
        ));
        assert!(prefer_gl_surface_config(
            false,
            Some(false),
            0,
            Some(true),
            8
        ));
    }
}
