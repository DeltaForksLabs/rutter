use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FakeGraphicsBackend {
    backend: BackendType,
}

#[derive(Debug, Default)]
struct RecordingBackendProbe {
    attempts: Vec<BackendType>,
    failing_backend: Option<BackendType>,
}

impl RecordingBackendProbe {
    fn failing(backend: BackendType) -> Self {
        Self {
            attempts: Vec::new(),
            failing_backend: Some(backend),
        }
    }

    fn initialize(&mut self, backend: BackendType) -> Result<FakeGraphicsBackend, BackendFailure> {
        self.attempts.push(backend);
        if self.failing_backend == Some(backend) {
            return Err(BackendFailure::new(
                backend,
                "named fake probe rejected backend",
            ));
        }
        Ok(FakeGraphicsBackend { backend })
    }
}

#[test]
fn retained_opengl_attempts_only_opengl_for_later_surfaces_and_reopenings() {
    let mut probe = RecordingBackendProbe::default();

    let secondary =
        initialize_required_backend(BackendType::OpenGl, |backend| probe.initialize(backend))
            .unwrap();
    let reopened =
        initialize_required_backend(secondary.backend, |backend| probe.initialize(backend))
            .unwrap();

    assert_eq!(secondary.backend, BackendType::OpenGl);
    assert_eq!(reopened.backend, BackendType::OpenGl);
    assert_eq!(
        probe.attempts,
        vec![BackendType::OpenGl, BackendType::OpenGl]
    );
    assert!(!probe.attempts.contains(&BackendType::Vulkan));
}

#[test]
fn retained_backend_failure_is_typed_and_does_not_probe_alternatives() {
    let mut probe = RecordingBackendProbe::failing(BackendType::OpenGl);

    let error =
        initialize_required_backend(BackendType::OpenGl, |backend| probe.initialize(backend))
            .unwrap_err();

    assert!(matches!(
        &error,
        GraphicsError::BackendInit(failure)
            if failure.backend == BackendType::OpenGl
                && failure.reason.contains("named fake probe rejected backend")
    ));
    assert_eq!(probe.attempts, vec![BackendType::OpenGl]);
    assert!(
        error
            .to_string()
            .contains("retained multi-window opengl backend initialization")
    );
    assert!(
        error
            .to_string()
            .contains("without probing backend alternatives")
    );
}
