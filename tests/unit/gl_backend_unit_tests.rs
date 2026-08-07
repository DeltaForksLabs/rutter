use std::cell::Cell;
use std::rc::Rc;

use winit::dpi::PhysicalSize;

use super::{
    BackendType, ContextActivation, GlOperationFailure, GraphicsError,
    ensure_backend_context_current, ensure_backend_context_not_current, prefer_gl_surface_config,
    select_preferred_candidate, validate_gl_surface_transparency, validated_gl_surface_dimensions,
};

struct FakeContextActivation {
    current: Cell<bool>,
    activation_count: Cell<usize>,
    activation_failure: Option<&'static str>,
}

impl FakeContextActivation {
    fn new(current: bool, activation_failure: Option<&'static str>) -> Self {
        Self {
            current: Cell::new(current),
            activation_count: Cell::new(0),
            activation_failure,
        }
    }
}

impl ContextActivation for FakeContextActivation {
    fn context_is_current(&self) -> bool {
        self.current.get()
    }

    fn is_current(&self) -> bool {
        self.current.get()
    }

    fn activate(&self) -> Result<(), String> {
        self.activation_count.set(self.activation_count.get() + 1);
        if let Some(reason) = self.activation_failure {
            return Err(reason.to_string());
        }
        self.current.set(true);
        Ok(())
    }

    fn deactivate(&self) -> Result<(), String> {
        if let Some(reason) = self.activation_failure {
            return Err(reason.to_string());
        }
        self.current.set(false);
        Ok(())
    }
}

struct SwitchingContextActivation {
    id: u8,
    current: Rc<Cell<Option<u8>>>,
    activation_count: Cell<usize>,
}

impl SwitchingContextActivation {
    fn new(id: u8, current: Rc<Cell<Option<u8>>>) -> Self {
        Self {
            id,
            current,
            activation_count: Cell::new(0),
        }
    }
}

impl ContextActivation for SwitchingContextActivation {
    fn context_is_current(&self) -> bool {
        self.current.get() == Some(self.id)
    }

    fn is_current(&self) -> bool {
        self.context_is_current()
    }

    fn activate(&self) -> Result<(), String> {
        self.activation_count.set(self.activation_count.get() + 1);
        self.current.set(Some(self.id));
        Ok(())
    }

    fn deactivate(&self) -> Result<(), String> {
        if self.is_current() {
            self.current.set(None);
        }
        Ok(())
    }
}

#[test]
fn already_current_context_is_not_reactivated() {
    let activation = FakeContextActivation::new(true, None);

    ensure_backend_context_current(&activation, "begin frame", GlOperationFailure::Frame).unwrap();

    assert_eq!(activation.activation_count.get(), 0);
}

#[test]
fn inactive_context_is_reactivated() {
    let activation = FakeContextActivation::new(false, None);

    ensure_backend_context_current(&activation, "begin frame", GlOperationFailure::Frame).unwrap();

    assert!(activation.current.get());
    assert_eq!(activation.activation_count.get(), 1);
}

#[test]
fn alternating_surfaces_rebind_their_matching_contexts() {
    let current = Rc::new(Cell::new(None));
    let first = SwitchingContextActivation::new(1, current.clone());
    let second = SwitchingContextActivation::new(2, current.clone());

    ensure_backend_context_current(&first, "first frame", GlOperationFailure::Frame).unwrap();
    ensure_backend_context_current(&second, "second frame", GlOperationFailure::Frame).unwrap();
    ensure_backend_context_current(&first, "first resize", GlOperationFailure::Resize).unwrap();

    assert_eq!(current.get(), Some(1));
    assert_eq!(first.activation_count.get(), 2);
    assert_eq!(second.activation_count.get(), 1);
}

#[test]
fn current_context_is_released_before_surface_resize() {
    let activation = FakeContextActivation::new(true, None);

    ensure_backend_context_not_current(
        &activation,
        "prepare backend resize",
        GlOperationFailure::Resize,
    )
    .unwrap();

    assert!(!activation.current.get());
}

#[test]
fn resize_deactivation_failure_is_typed() {
    let activation = FakeContextActivation::new(true, Some("release blocked"));
    let error = ensure_backend_context_not_current(
        &activation,
        "prepare backend resize",
        GlOperationFailure::Resize,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        GraphicsError::Resize { backend: BackendType::OpenGl, reason }
            if reason.contains("release blocked") && reason.contains("expected the context to be not current")
    ));
}

#[test]
fn activation_failure_is_reported_as_a_typed_frame_error() {
    let activation = FakeContextActivation::new(false, Some("activation blocked"));
    let error =
        ensure_backend_context_current(&activation, "begin frame", GlOperationFailure::Frame)
            .unwrap_err();

    let GraphicsError::Frame { backend, reason } = error else {
        panic!("expected typed OpenGL frame error");
    };
    assert_eq!(backend, BackendType::OpenGl);
    assert!(reason.contains("begin frame"));
    assert!(reason.contains("activation blocked"));
    assert!(reason.contains("expected the backend OpenGL context and window surface"));
}

#[test]
fn resize_activation_failure_preserves_the_operation_type() {
    let activation = FakeContextActivation::new(false, Some("wrong drawable"));
    let error = ensure_backend_context_current(&activation, "resize", GlOperationFailure::Resize)
        .unwrap_err();

    assert!(matches!(
        error,
        GraphicsError::Resize { backend: BackendType::OpenGl, reason }
            if reason.contains("resize") && reason.contains("wrong drawable")
    ));
}

#[test]
fn resize_dimensions_are_validated_from_the_received_size() {
    assert_eq!(
        validated_gl_surface_dimensions(PhysicalSize::new(900, 700)).unwrap(),
        (900, 700)
    );
    let error = validated_gl_surface_dimensions(PhysicalSize::new(u32::MAX, 700)).unwrap_err();
    assert!(error.contains(&u32::MAX.to_string()));
    assert!(error.contains("expected a value representable as i32"));
}

#[test]
fn empty_config_candidates_return_none_without_a_picker_panic() {
    let empty = std::iter::empty::<u8>();
    assert_eq!(select_preferred_candidate(empty, |_, _| true), None);

    let candidates = vec![4_u8, 2, 7];
    let selected =
        select_preferred_candidate(candidates.into_iter(), |next, current| next < current);
    assert_eq!(selected, Some(2));
}

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
