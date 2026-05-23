//! MCP server lifecycle management.

mod agent;
mod logs;
mod manager;
pub mod proxy;
mod registry;

pub use agent::{ActionKind, ActionSink, ActionStatus, AgentAction};
pub use logs::{LogEntry, LogSink, LogStream};
pub use manager::McpManager;
pub use registry::ServerRegistry;
