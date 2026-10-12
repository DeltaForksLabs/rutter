//! Configured metrics are observed on the runner thread after native dispatch.
use super::*;
use crate::multi_window::SurfaceMetrics;

impl<A: MultiWindowAppLogic + 'static> MultiWindowRunner<A> {
    pub(super) fn notify_surface_metrics(&mut self, surface: SurfaceId) {
        let Some(window) = self
            .surface_runners
            .get(&surface)
            .and_then(RutterRunner::native_window)
        else {
            return;
        };
        let size = window.surface_size();
        let scale = window.scale_factor();
        let logical_size = window.surface_logical_size();
        let metrics = SurfaceMetrics {
            physical_size: (size.width, size.height),
            logical_size: (logical_size.width, logical_size.height),
            scale_factor: scale,
            output_name: window.layer_output_name(),
            layer_shell_version: window.layer_shell_version(),
        };
        if self.surface_metrics.get(&surface) == Some(&metrics) {
            return;
        }
        self.surface_metrics.insert(surface, metrics.clone());
        A::surface_metrics_changed(&mut self.canonical_state, surface, metrics);
        self.revision += 1;
        self.publish_surface_state(self.canonical_state.clone(), self.revision);
    }
}
