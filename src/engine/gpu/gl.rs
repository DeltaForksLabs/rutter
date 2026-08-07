// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::ffi::{CString, c_void};
use std::num::NonZeroU32;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any, resume_unwind};
use std::ptr;
use std::rc::Rc;

use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, PossiblyCurrentGlContext, Version,
};
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

const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
type GlGetInteger = unsafe extern "system" fn(u32, *mut i32);

#[derive(Debug)]
struct EmptyGlConfigSet;

pub struct GlBackend {
    skia_surface: SkiaSurface,
    skia_context: DirectContext,
    gl_surface: GlutinSurface<WindowSurface>,
    gl_context: PossiblyCurrentContext,
    window: Rc<Window>,
    fb_info: FramebufferInfo,
    sample_count: usize,
    stencil_size: usize,
    get_integer: GlGetInteger,
}

trait ContextActivation {
    fn context_is_current(&self) -> bool;
    fn is_current(&self) -> bool;
    fn activate(&self) -> Result<(), String>;
    fn deactivate(&self) -> Result<(), String>;
}

struct GlutinContextActivation<'a> {
    context: &'a PossiblyCurrentContext,
    surface: &'a GlutinSurface<WindowSurface>,
}

impl ContextActivation for GlutinContextActivation<'_> {
    fn context_is_current(&self) -> bool {
        self.context.is_current()
    }

    fn is_current(&self) -> bool {
        self.context_is_current() && self.surface.is_current(self.context)
    }

    fn activate(&self) -> Result<(), String> {
        self.context
            .make_current(self.surface)
            .map_err(|err| err.to_string())
    }

    fn deactivate(&self) -> Result<(), String> {
        self.context
            .make_not_current_in_place()
            .map_err(|err| err.to_string())
    }
}

#[derive(Clone, Copy)]
enum GlOperationFailure {
    Frame,
    Resize,
}

impl GlOperationFailure {
    fn graphics_error(self, reason: String) -> GraphicsError {
        match self {
            Self::Frame => GraphicsError::Frame {
                backend: BackendType::OpenGl,
                reason,
            },
            Self::Resize => GraphicsError::Resize {
                backend: BackendType::OpenGl,
                reason,
            },
        }
    }
}

fn ensure_backend_context_current(
    activation: &impl ContextActivation,
    operation: &'static str,
    failure: GlOperationFailure,
) -> Result<(), GraphicsError> {
    if activation.is_current() {
        return Ok(());
    }
    activation.activate().map_err(|reason| {
        failure.graphics_error(format!(
            "{operation}: failed to make the backend OpenGL context and window surface current: {reason}; expected the backend OpenGL context and window surface to be current"
        ))
    })
}

fn ensure_backend_context_not_current(
    activation: &impl ContextActivation,
    operation: &'static str,
    failure: GlOperationFailure,
) -> Result<(), GraphicsError> {
    if !activation.context_is_current() {
        return Ok(());
    }
    activation.deactivate().map_err(|reason| {
        failure.graphics_error(format!(
            "{operation}: failed to release the backend OpenGL context: {reason}; expected the context to be not current"
        ))
    })
}

impl GlBackend {
    pub fn try_new(
        event_loop: &ActiveEventLoop,
        attrs: WindowAttributes,
    ) -> Result<Box<dyn GraphicsBackend>, BackendFailure> {
        let transparent = attrs.transparent();
        let (window, gl_config) = build_gl_window(event_loop, attrs, transparent)?;
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
        let not_current = match unsafe { display.create_context(&gl_config, &context_attributes) } {
            Ok(context) => context,
            Err(desktop_error) => unsafe {
                display.create_context(&gl_config, &fallback_context_attributes)
            }
            .map_err(|gles_error| {
                BackendFailure::new(
                    BackendType::OpenGl,
                    format!(
                        "desktop OpenGL context failed: {desktop_error}; GLES 3.0 context failed: {gles_error}"
                    ),
                )
            })?,
        };

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

        let get_integer = load_gl_get_integer(&display)?;
        let fb_info = current_framebuffer_info(get_integer);
        let sample_count = gl_config.num_samples() as usize;
        let stencil_size = gl_config.stencil_size() as usize;
        let skia_surface =
            create_skia_surface(size, &mut skia_context, fb_info, sample_count, stencil_size)
                .map_err(|reason| BackendFailure::new(BackendType::OpenGl, reason))?;

        Ok(Box::new(Self {
            skia_surface,
            skia_context,
            gl_context,
            gl_surface,
            window,
            fb_info,
            sample_count,
            stencil_size,
            get_integer,
        }))
    }

    fn ensure_current(
        &self,
        operation: &'static str,
        failure: GlOperationFailure,
    ) -> Result<(), GraphicsError> {
        let activation = GlutinContextActivation {
            context: &self.gl_context,
            surface: &self.gl_surface,
        };
        ensure_backend_context_current(&activation, operation, failure)
    }

    fn ensure_not_current(
        &self,
        operation: &'static str,
        failure: GlOperationFailure,
    ) -> Result<(), GraphicsError> {
        let activation = GlutinContextActivation {
            context: &self.gl_context,
            surface: &self.gl_surface,
        };
        ensure_backend_context_not_current(&activation, operation, failure)
    }
}

fn build_gl_window(
    event_loop: &ActiveEventLoop,
    attrs: WindowAttributes,
    transparent: bool,
) -> Result<(Option<Window>, Config), BackendFailure> {
    let builder = DisplayBuilder::new().with_window_attributes(Some(attrs));
    // glutin-winit 0.5 makes the picker infallible, so a private sentinel is
    // recovered here to preserve backend fallback when EGL reports zero configs.
    let attempt = catch_unwind(AssertUnwindSafe(|| {
        builder.build(event_loop, ConfigTemplateBuilder::new(), |configs| {
            select_gl_config(transparent, configs).unwrap_or_else(|| panic_any(EmptyGlConfigSet))
        })
    }));
    match attempt {
        Ok(result) => {
            result.map_err(|error| BackendFailure::new(BackendType::OpenGl, error.to_string()))
        }
        Err(payload) if payload.is::<EmptyGlConfigSet>() => Err(BackendFailure::new(
            BackendType::OpenGl,
            "glutin returned zero GL configs; expected at least one compatible configuration",
        )),
        Err(payload) => resume_unwind(payload),
    }
}

fn select_gl_config(
    transparent: bool,
    configs: Box<dyn Iterator<Item = Config> + '_>,
) -> Option<Config> {
    select_preferred_candidate(configs, |candidate, current| {
        prefer_gl_surface_config(
            transparent,
            candidate.supports_transparency(),
            candidate.num_samples(),
            current.supports_transparency(),
            current.num_samples(),
        )
    })
}

fn select_preferred_candidate<Candidate>(
    mut candidates: impl Iterator<Item = Candidate>,
    prefer: impl Fn(&Candidate, &Candidate) -> bool,
) -> Option<Candidate> {
    let mut selected = candidates.next()?;
    for candidate in candidates {
        if prefer(&candidate, &selected) {
            selected = candidate;
        }
    }
    Some(selected)
}

fn load_gl_get_integer(display: &impl GlDisplay) -> Result<GlGetInteger, BackendFailure> {
    let symbol = CString::new("glGetIntegerv").unwrap();
    let address = display.get_proc_address(symbol.as_c_str());
    if address.is_null() {
        return Err(BackendFailure::new(
            BackendType::OpenGl,
            "OpenGL symbol `glGetIntegerv` is null; expected a callable context-local symbol",
        ));
    }
    Ok(unsafe { std::mem::transmute::<*const c_void, GlGetInteger>(address) })
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

fn current_framebuffer_info(get_integer: GlGetInteger) -> FramebufferInfo {
    let mut framebuffer_id = 0_i32;
    unsafe {
        get_integer(GL_FRAMEBUFFER_BINDING, &mut framebuffer_id);
    }

    FramebufferInfo {
        fboid: framebuffer_id as u32,
        format: gpu::gl::Format::RGBA8.into(),
        ..Default::default()
    }
}

fn create_skia_surface(
    size: PhysicalSize<u32>,
    skia_context: &mut DirectContext,
    fb_info: FramebufferInfo,
    sample_count: usize,
    stencil_size: usize,
) -> Result<SkiaSurface, String> {
    let dimensions = validated_gl_surface_dimensions(size)?;
    let render_target =
        backend_render_targets::make_gl(dimensions, sample_count, stencil_size, fb_info);
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

fn validated_gl_surface_dimensions(size: PhysicalSize<u32>) -> Result<(i32, i32), String> {
    let width = size.width.try_into().map_err(|_| {
        format!(
            "OpenGL surface width `{}` is invalid; expected a value representable as i32",
            size.width
        )
    })?;
    let height = size.height.try_into().map_err(|_| {
        format!(
            "OpenGL surface height `{}` is invalid; expected a value representable as i32",
            size.height
        )
    })?;
    Ok((width, height))
}

impl GraphicsBackend for GlBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::OpenGl
    }

    fn begin_frame(&mut self) -> Result<&Canvas, GraphicsError> {
        self.ensure_current("begin frame", GlOperationFailure::Frame)?;
        Ok(self.skia_surface.canvas())
    }

    fn end_frame(&mut self) -> Result<(), GraphicsError> {
        self.ensure_current("end frame", GlOperationFailure::Frame)?;
        self.ensure_current("flush and submit frame", GlOperationFailure::Frame)?;
        self.skia_context
            .flush_and_submit_surface(&mut self.skia_surface, None);
        self.ensure_current("swap frame buffers", GlOperationFailure::Frame)?;
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .map_err(|err| GraphicsError::Frame {
                backend: BackendType::OpenGl,
                reason: format!("swap frame buffers: {err}"),
            })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) -> Result<(), GraphicsError> {
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        // glutin 0.32 requires resize before make_current on Wayland because
        // activation can latch the old back buffer until the next swap.
        self.ensure_not_current("prepare backend resize", GlOperationFailure::Resize)?;
        self.gl_surface.resize(
            &self.gl_context,
            NonZeroU32::new(size.width.max(1)).unwrap(),
            NonZeroU32::new(size.height.max(1)).unwrap(),
        );
        self.ensure_current("activate resized backend", GlOperationFailure::Resize)?;
        self.ensure_current(
            "query framebuffer during resize",
            GlOperationFailure::Resize,
        )?;
        self.fb_info = current_framebuffer_info(self.get_integer);
        self.skia_surface = create_skia_surface(
            size,
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
}

impl Drop for GlBackend {
    fn drop(&mut self) {
        // Skia must release driver resources under this backend's context, while
        // glutin must receive a non-current context before native teardown.
        let activated = self
            .ensure_current("release backend resources", GlOperationFailure::Frame)
            .is_ok();
        if !activated {
            self.skia_context.abandon();
        } else {
            self.skia_context.flush_and_submit();
            self.skia_context.release_resources_and_abandon();
        }
        let _ = self.ensure_not_current("release backend context", GlOperationFailure::Frame);
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/gl_backend_unit_tests.rs"]
mod tests;
