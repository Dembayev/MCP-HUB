//! Tauri commands — the IPC surface the React frontend talks to.
//!
//! Each module groups commands by domain. Keep command bodies thin: they
//! validate input, delegate to services on `AppState`, and translate errors
//! to `AppError` (which serializes to a string for the frontend).

pub mod permissions;
pub mod servers;
pub mod system;
