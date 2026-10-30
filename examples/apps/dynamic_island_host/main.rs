//! Activity host: `<existing-private-0700-directory> [--overlay]`.
//! The optional native overlay requires Wayland layer-shell with OnDemand support;
//! unsupported native backends/protocols return the runner's explicit error.

#[cfg(unix)]
mod app;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    return app::run();
    #[cfg(not(unix))]
    Err("dynamic_island_host requires Unix activity IPC".into())
}
