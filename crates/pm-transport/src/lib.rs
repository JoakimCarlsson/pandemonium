//! The transports that reach a server that is not simply listening on this machine: inside a Windows Subsystem for Linux distribution, inside a dev container, and through a tunnel that opens no port.
//!
//! Each hands the client a [`Target`](pm_protocol::Target), a local address and a token, and everything above it is unchanged. The first two start the server themselves, because nothing is running there yet.

mod bridge;
mod container;
mod devcontainer;
mod provision;
mod tunnel;
mod wsl;

pub use bridge::Progress;
pub use bridge::bridge;
pub use container::container;
pub use tunnel::tunnel;
pub use wsl::wsl;
