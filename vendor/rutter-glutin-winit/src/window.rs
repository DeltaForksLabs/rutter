use std::num::NonZeroU32;

use glutin::context::PossiblyCurrentContext;
use glutin::surface::{
    GlSurface, ResizeableSurface, Surface, SurfaceAttributes, SurfaceAttributesBuilder,
    SurfaceTypeTrait, WindowSurface,
};
use raw_window_handle::{HandleError, HasWindowHandle};
use winit::window::Window;

/// [`Window`] extensions for working with [`glutin`] surfaces.
pub trait GlWindow {
    /// Build attributes for a GL window surface matching the renderable surface.
    ///
    /// # Panics
    /// Panics if either surface dimension is zero.
    ///
    /// # Example
    /// ```no_run
    /// use glutin_winit::GlWindow;
    /// # use winit::window::Window;
    /// # fn example(winit_window: &dyn Window) {
    /// let attrs = winit_window.build_surface_attributes(<_>::default());
    /// # let _ = attrs;
    /// # }
    /// ```
    fn build_surface_attributes(
        &self,
        builder: SurfaceAttributesBuilder<WindowSurface>,
    ) -> Result<SurfaceAttributes<WindowSurface>, HandleError>;

    /// Resize the GL surface to the window's renderable surface size.
    /// No-op if either dimension is zero.
    ///
    /// # Example
    /// ```no_run
    /// use glutin_winit::GlWindow;
    /// # use glutin::surface::{Surface, WindowSurface};
    /// # use winit::window::Window;
    /// # fn example(winit_window: &dyn Window, gl_surface: &Surface<WindowSurface>, gl_context: &glutin::context::PossiblyCurrentContext) {
    /// winit_window.resize_surface(gl_surface, gl_context);
    /// # }
    /// ```
    fn resize_surface(
        &self,
        surface: &Surface<impl SurfaceTypeTrait + ResizeableSurface>,
        context: &PossiblyCurrentContext,
    );
}

impl GlWindow for dyn Window + '_ {
    fn build_surface_attributes(
        &self,
        builder: SurfaceAttributesBuilder<WindowSurface>,
    ) -> Result<SurfaceAttributes<WindowSurface>, HandleError> {
        let (width, height) = self
            .surface_size()
            .non_zero()
            .expect("invalid zero surface size");
        let handle = self.window_handle()?.as_raw();
        Ok(builder.build(handle, width, height))
    }

    fn resize_surface(
        &self,
        surface: &Surface<impl SurfaceTypeTrait + ResizeableSurface>,
        context: &PossiblyCurrentContext,
    ) {
        if let Some((width, height)) = self.surface_size().non_zero() {
            surface.resize(context, width, height);
        }
    }
}

trait NonZeroU32PhysicalSize {
    fn non_zero(self) -> Option<(NonZeroU32, NonZeroU32)>;
}

impl NonZeroU32PhysicalSize for winit::dpi::PhysicalSize<u32> {
    fn non_zero(self) -> Option<(NonZeroU32, NonZeroU32)> {
        let width = NonZeroU32::new(self.width)?;
        let height = NonZeroU32::new(self.height)?;
        Some((width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::NonZeroU32PhysicalSize;
    use winit::dpi::PhysicalSize;

    #[test]
    fn surface_dimensions_must_both_be_nonzero() {
        assert!(PhysicalSize::new(0, 10).non_zero().is_none());
        assert!(PhysicalSize::new(10, 0).non_zero().is_none());
        let (width, height) = PhysicalSize::new(40, 30).non_zero().unwrap();
        assert_eq!(width.get(), 40);
        assert_eq!(height.get(), 30);
    }
}
