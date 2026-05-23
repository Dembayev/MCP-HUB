//! `mcp-hub-proxy` — tiny stdio ↔ Unix-socket bridge.
//!
//! This is the executable an MCP client (Claude Desktop, Cursor, …) spawns
//! when it wants to talk to a server through MCP Hub. It does nothing
//! "smart" — it connects to the running Hub via a Unix domain socket,
//! announces which server the client wants, and then splices its own
//! stdin/stdout through the socket. All observation, classification, and
//! enforcement happens in the Hub.
//!
//! Usage:
//!   mcp-hub-proxy <server-id>
//!
//! The Hub creates the socket at `$XDG_DATA_HOME/MCP Hub/proxy.sock`
//! (macOS: `~/Library/Application Support/MCP Hub/proxy.sock`).

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    use tokio::io::{AsyncWriteExt, AsyncReadExt};
    use tokio::net::UnixStream;

    let args: Vec<String> = env::args().collect();
    let Some(server_id) = args.get(1).cloned() else {
        eprintln!("mcp-hub-proxy: missing server id\nusage: mcp-hub-proxy <server-id>");
        return ExitCode::from(2);
    };

    let sock_path = match resolve_sock_path() {
        Some(p) => p,
        None => {
            eprintln!("mcp-hub-proxy: could not resolve MCP Hub data directory");
            return ExitCode::from(3);
        }
    };

    let stream = match UnixStream::connect(&sock_path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "mcp-hub-proxy: cannot connect to MCP Hub at {}: {}\n  Is the MCP Hub app running?",
                sock_path.display(),
                e
            );
            return ExitCode::from(4);
        }
    };

    let (mut sock_reader, mut sock_writer) = stream.into_split();

    // Greeting: announce the requested server.
    let mut greeting = server_id.into_bytes();
    greeting.push(b'\n');
    if let Err(e) = sock_writer.write_all(&greeting).await {
        eprintln!("mcp-hub-proxy: greeting failed: {}", e);
        return ExitCode::from(5);
    }

    // Bidirectional splice. stdin → socket, socket → stdout. Use a small
    // 8 KB buffer; MCP NDJSON messages are short so we don't need more.
    let stdin_to_socket = async {
        let mut stdin = tokio::io::stdin();
        let _ = tokio::io::copy(&mut stdin, &mut sock_writer).await;
        let _ = sock_writer.shutdown().await;
    };

    let socket_to_stdout = async {
        let mut stdout = tokio::io::stdout();
        let mut buf = [0u8; 8 * 1024];
        loop {
            match sock_reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if stdout.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                    if stdout.flush().await.is_err() {
                        break;
                    }
                }
            }
        }
    };

    tokio::select! {
        _ = stdin_to_socket => {},
        _ = socket_to_stdout => {},
    };

    ExitCode::SUCCESS
}

#[cfg(not(unix))]
fn main() -> ExitCode {
    eprintln!("mcp-hub-proxy: this build targets Unix only (Windows support coming via named pipes).");
    ExitCode::from(1)
}

#[cfg(unix)]
fn resolve_sock_path() -> Option<PathBuf> {
    // Honor MCP_HUB_PROXY_SOCK for tests / non-default installs, otherwise
    // mirror the Hub's data directory convention.
    if let Ok(custom) = env::var("MCP_HUB_PROXY_SOCK") {
        return Some(PathBuf::from(custom));
    }
    Some(dirs::data_dir()?.join("MCP Hub").join("proxy.sock"))
}
