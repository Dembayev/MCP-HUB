//! System / meta information commands.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub data_dir: String,
    /// Platform sandbox label — one of `macos-sandbox-exec`, `noop`.
    /// Lets the UI render a "Sandbox: enforced" / "not enforced" badge
    /// without doing OS detection client-side.
    pub sandbox_enforcement: &'static str,
}

#[tauri::command]
pub async fn app_info(state: State<'_, Arc<AppState>>) -> AppResult<AppInfo> {
    let sandbox_enforcement = if cfg!(target_os = "macos") {
        "macos-sandbox-exec"
    } else {
        "noop"
    };
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: state.data_dir.to_string_lossy().to_string(),
        sandbox_enforcement,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    pub server_id: String,
    pub server_name: String,
    /// Absolute path to the `mcp-hub-proxy` helper binary.
    pub proxy_path: String,
    /// Pre-formatted JSON snippet for `claude_desktop_config.json`.
    pub snippet: String,
    pub sock_path: String,
}

/// Build the JSON config snippet a user can paste into Claude Desktop /
/// Cursor / etc. to route their traffic through MCP Hub for a given server.
#[tauri::command]
pub async fn get_proxy_config(
    state: State<'_, Arc<AppState>>,
    server_id: String,
) -> AppResult<ProxyConfig> {
    let server = state.mcp.registry().get(&server_id)?;

    let proxy_path = resolve_proxy_binary().ok_or_else(|| {
        AppError::Internal("could not locate the mcp-hub-proxy binary".into())
    })?;
    let proxy_path_str = proxy_path.to_string_lossy().into_owned();
    let sock_path = state.data_dir.join("proxy.sock").to_string_lossy().into_owned();

    let entry_key = sanitize_key(&server.name);
    let snippet = format!(
        r#"{{
  "mcpServers": {{
    "{key}": {{
      "command": "{cmd}",
      "args": ["{id}"]
    }}
  }}
}}"#,
        key = entry_key,
        cmd = escape_json_string(&proxy_path_str),
        id = escape_json_string(&server.id),
    );

    Ok(ProxyConfig {
        server_id: server.id,
        server_name: server.name,
        proxy_path: proxy_path_str,
        snippet,
        sock_path,
    })
}

/// Locate the `mcp-hub-proxy` helper binary that ships alongside the main
/// app binary. In dev that's `target/<profile>/mcp-hub-proxy`; in a bundled
/// app that's a sibling inside `Contents/MacOS/` (macOS) or the same dir
/// (Linux/Windows).
fn resolve_proxy_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) {
        "mcp-hub-proxy.exe"
    } else {
        "mcp-hub-proxy"
    };
    Some(dir.join(name))
}

fn sanitize_key(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .to_lowercase()
}

fn escape_json_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
