//! This library provides helpers for cross-platform [`glutin`] bootstrapping
//! with [`winit`].
//!
//! Adapted from glutin-winit 0.5.0 for rutter-winit 0.31.0-beta.3.

#![deny(rust_2018_idioms)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::all)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![cfg_attr(clippy, deny(warnings))]

mod window;

pub use window::GlWindow;

use std::error::Error;

use glutin::config::{Config, ConfigTemplateBuilder};
use glutin::display::{Display, DisplayApiPreference};
#[cfg(x11_platform)]
use glutin::platform::x11::X11GlConfigExt;
use glutin::prelude::*;

#[cfg(wgl_backend)]
use raw_window_handle::HasWindowHandle;
use raw_window_handle::{HasDisplayHandle, RawWindowHandle};
#[cfg(x11_platform)]
use winit::error::NotSupportedError;
use winit::error::RequestError;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

#[cfg(glx_backend)]
use winit::platform::x11::register_xlib_error_hook;
#[cfg(x11_platform)]
use winit::platform::x11::WindowAttributesX11;

#[cfg(all(not(egl_backend), not(glx_backend), not(wgl_backend), not(cgl_backend)))]
compile_error!("Please select at least one api backend");

/// A selected GL config and the optional window created for it.
pub type WindowAndConfig = (Option<Box<dyn Window>>, Config);

/// Creates a [`Display`] and an optional compatible window with the selected
/// [`Config`]. Additional windows can be made using [`finalize_window`].
#[derive(Default, Debug, Clone)]
pub struct DisplayBuilder {
    preference: ApiPreference,
    window_attributes: Option<WindowAttributes>,
}

impl DisplayBuilder {
    /// Create a new display builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the preferred API for creating the display.
    pub fn with_preference(mut self, preference: ApiPreference) -> Self {
        self.preference = preference;
        self
    }

    /// Set attributes for an optional window created with the display.
    pub fn with_window_attributes(mut self, window_attributes: Option<WindowAttributes>) -> Self {
        self.window_attributes = window_attributes;
        self
    }

    /// Initialize the OpenGL platform, select a configuration and optionally
    /// create a compatible window. For WGL, pass window attributes to obtain
    /// modern OpenGL features. On platforms without an initial window (such as
    /// Android), use [`finalize_window`] later with the selected config.
    pub fn build<Picker>(
        mut self,
        event_loop: &dyn ActiveEventLoop,
        template_builder: ConfigTemplateBuilder,
        config_picker: Picker,
    ) -> Result<WindowAndConfig, Box<dyn Error>>
    where
        Picker: FnOnce(Box<dyn Iterator<Item = Config> + '_>) -> Config,
    {
        // WGL needs the native window before selecting its GL config.
        #[cfg(wgl_backend)]
        let window = if let Some(attributes) = self.window_attributes.take() {
            Some(event_loop.create_window(attributes)?)
        } else {
            None
        };

        #[cfg(wgl_backend)]
        let raw_window_handle = window
            .as_ref()
            .and_then(|window| window.window_handle().ok())
            .map(|handle| handle.as_raw());
        #[cfg(not(wgl_backend))]
        let raw_window_handle = None;

        let gl_display = create_display(event_loop, self.preference, raw_window_handle)?;

        #[cfg(wgl_backend)]
        let template_builder = if let Some(handle) = raw_window_handle {
            template_builder.compatible_with_native_window(handle)
        } else {
            template_builder
        };

        let template = template_builder.build();
        let gl_config = unsafe {
            let configs = gl_display.find_configs(template)?;
            config_picker(configs)
        };

        #[cfg(not(wgl_backend))]
        let window = if let Some(attributes) = self.window_attributes.take() {
            Some(finalize_window(event_loop, attributes, &gl_config)?)
        } else {
            None
        };

        Ok((window, gl_config))
    }
}

fn create_display(
    event_loop: &dyn ActiveEventLoop,
    _api_preference: ApiPreference,
    _raw_window_handle: Option<RawWindowHandle>,
) -> Result<Display, Box<dyn Error>> {
    #[cfg(egl_backend)]
    let _preference = DisplayApiPreference::Egl;

    #[cfg(glx_backend)]
    let _preference = DisplayApiPreference::Glx(Box::new(register_xlib_error_hook));

    #[cfg(cgl_backend)]
    let _preference = DisplayApiPreference::Cgl;

    #[cfg(wgl_backend)]
    let _preference = DisplayApiPreference::Wgl(_raw_window_handle);

    #[cfg(all(egl_backend, glx_backend))]
    let _preference = match _api_preference {
        ApiPreference::PreferEgl => {
            DisplayApiPreference::EglThenGlx(Box::new(register_xlib_error_hook))
        }
        ApiPreference::FallbackEgl => {
            DisplayApiPreference::GlxThenEgl(Box::new(register_xlib_error_hook))
        }
    };

    #[cfg(all(wgl_backend, egl_backend))]
    let _preference = match _api_preference {
        ApiPreference::PreferEgl => DisplayApiPreference::EglThenWgl(_raw_window_handle),
        ApiPreference::FallbackEgl => DisplayApiPreference::WglThenEgl(_raw_window_handle),
    };

    let handle = event_loop.display_handle()?.as_raw();
    unsafe { Ok(Display::new(handle, _preference)?) }
}

/// Finalize creation of a window compatible with the selected [`Config`].
/// When the config does not support transparency, the window is made opaque.
/// On X11, an incompatible non-X11 platform-attributes object is rejected
/// rather than discarded when a specific GLX visual is required.
pub fn finalize_window(
    event_loop: &dyn ActiveEventLoop,
    mut attributes: WindowAttributes,
    gl_config: &Config,
) -> Result<Box<dyn Window>, RequestError> {
    if gl_config.supports_transparency() == Some(false) {
        attributes = attributes.with_transparent(false);
    }

    #[cfg(x11_platform)]
    if let Some(x11_visual) = gl_config.x11_visual() {
        attributes = with_x11_visual(attributes, x11_visual.visual_id() as _)?;
    }

    event_loop.create_window(attributes)
}

#[cfg(x11_platform)]
fn with_x11_visual(
    mut attributes: WindowAttributes,
    visual_id: u32,
) -> Result<WindowAttributes, RequestError> {
    match attributes.platform.as_mut() {
        Some(platform) => {
            let x11_attributes = platform.cast_mut::<WindowAttributesX11>().ok_or_else(|| {
                RequestError::NotSupported(NotSupportedError::new(
                    "GLX visual requires X11 window attributes, but another platform's attributes were supplied",
                ))
            })?;
            *x11_attributes = x11_attributes.clone().with_x11_visual(visual_id as _);
        }
        None => {
            attributes = attributes.with_platform_attributes(Box::new(
                WindowAttributesX11::default().with_x11_visual(visual_id as _),
            ));
        }
    }
    Ok(attributes)
}

/// Simplified [`DisplayApiPreference`] for cross-platform window creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApiPreference {
    /// Prefer EGL to GLX/WGL when both are available.
    PreferEgl,

    /// Try GLX/WGL first, then fall back to EGL.
    #[default]
    FallbackEgl,
}

#[cfg(all(test, x11_platform))]
mod tests {
    use super::*;
    use winit::window::PlatformWindowAttributes;

    #[derive(Clone, Debug)]
    struct OtherPlatformAttributes;

    impl PlatformWindowAttributes for OtherPlatformAttributes {
        fn box_clone(&self) -> Box<dyn PlatformWindowAttributes> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn visual_keeps_existing_x11_attributes() {
        let attributes = WindowAttributes::default().with_platform_attributes(Box::new(
            WindowAttributesX11::default().with_name("rutter", "popup"),
        ));
        let attributes = with_x11_visual(attributes, 42).unwrap();
        let platform = attributes.platform.unwrap();
        let x11 = platform.cast_ref::<WindowAttributesX11>().unwrap();
        let debug = format!("{x11:?}");
        assert!(debug.contains("rutter"));
        assert!(debug.contains("popup"));
        assert!(debug.contains("42"));
    }

    #[test]
    fn visual_refuses_unrelated_platform_attributes() {
        let attributes =
            WindowAttributes::default().with_platform_attributes(Box::new(OtherPlatformAttributes));
        let error = with_x11_visual(attributes, 42).unwrap_err();
        assert!(matches!(error, RequestError::NotSupported(_)));
    }

    #[test]
    fn visual_adds_x11_attributes_when_none_were_given() {
        let attributes = with_x11_visual(WindowAttributes::default(), 42).unwrap();
        assert!(attributes
            .platform
            .unwrap()
            .cast_ref::<WindowAttributesX11>()
            .is_some());
    }
}
