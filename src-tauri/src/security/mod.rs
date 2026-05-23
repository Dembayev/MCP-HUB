//! Permissions and sandbox policy.
//!
//! Today this module models a permission as a `(scope, target)` pair (e.g.
//! `fs.read` + `/Users/me/projects/**`). The MVP will surface granted
//! permissions in the UI and persist user grants; the actual enforcement
//! mechanism (seatbelt on macOS, AppArmor on Linux, job objects on Windows)
//! will plug in here behind the `Sandbox` trait.

pub mod approvals;
pub mod permissions;
pub mod sandbox;

pub use approvals::{
    risk_for_kind, ApprovalDecision, ApprovalRegistry, ApprovalRequest,
};
pub use permissions::{Permission, PermissionScope};
pub use sandbox::{for_current_platform, NoopSandbox, Sandbox, SandboxedCommand};
