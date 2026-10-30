//! Simulated producer: `<socket> download|upload|media [--unknown-total]`.
//! No network transfer, media player, shell command, or artwork is executed.

#[cfg(unix)]
mod producer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    return producer::run();
    #[cfg(not(unix))]
    Err("dynamic_island_client requires Unix activity IPC".into())
}
