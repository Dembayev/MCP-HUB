//! Sandbox abstraction with platform-specific enforcement.
//!
//! The MVP target is **macOS sandbox-exec**. On other platforms we ship a
//! [`NoopSandbox`] so dev/build works everywhere, with the same trait
//! shape — Linux (AppArmor / namespaces) and Windows (job objects) will
//! plug in behind the same `Sandbox::prepare` later.
//!
//! ## Threat model the MVP defends against
//!
//! Today MCP servers run with the user's full permissions. A malicious or
//! buggy server can read `~/.ssh`, exfiltrate via any URL, spawn anything.
//! The MVP narrows that to **what the user explicitly granted at install
//! time** for filesystem and network. It is not kernel-grade isolation —
//! it's "visually convincing enforcement" that closes the obvious risks
//! and gives MCP Hub a credible security story for launch.

use crate::db::models::McpServer;
use crate::db::permissions::PersistedPermission;
use crate::error::AppResult;

/// A prepared command — possibly wrapped by a sandbox launcher.
#[derive(Debug, Clone)]
pub struct SandboxedCommand {
    pub program: String,
    pub args: Vec<String>,
    /// Human-readable label for logs / UI ("sandbox-exec", "noop").
    pub enforcement: &'static str,
}

pub trait Sandbox: Send + Sync {
    /// Wrap the given server's command with whatever platform-specific
    /// isolation we ship. `profiles_dir` is a writable scratch area the
    /// sandbox can drop generated profile files into.
    fn prepare(
        &self,
        server: &McpServer,
        permissions: &[PersistedPermission],
        profiles_dir: &std::path::Path,
    ) -> AppResult<SandboxedCommand>;
}

/// Choose the right Sandbox impl for the current OS.
///
/// Honors `MCP_HUB_SANDBOX=off` for ad-hoc debugging — useful when a
/// server fails to start under the profile and you need to isolate
/// whether the sandbox is at fault.
pub fn for_current_platform() -> Box<dyn Sandbox> {
    if matches!(std::env::var("MCP_HUB_SANDBOX").as_deref(), Ok("off") | Ok("0")) {
        tracing::warn!("MCP_HUB_SANDBOX=off — sandbox enforcement disabled");
        return Box::new(NoopSandbox);
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(MacosSandbox)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(NoopSandbox)
    }
}

// ---------------------------------------------------------------------------
// NoopSandbox — used on Linux/Windows until real impls land.
// ---------------------------------------------------------------------------

pub struct NoopSandbox;

impl Sandbox for NoopSandbox {
    fn prepare(
        &self,
        server: &McpServer,
        _permissions: &[PersistedPermission],
        _profiles_dir: &std::path::Path,
    ) -> AppResult<SandboxedCommand> {
        Ok(SandboxedCommand {
            program: server.command.clone(),
            args: server.args.clone(),
            enforcement: "noop",
        })
    }
}

// ---------------------------------------------------------------------------
// MacosSandbox — wraps the child in `sandbox-exec -f <profile>`.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub struct MacosSandbox;

#[cfg(target_os = "macos")]
impl Sandbox for MacosSandbox {
    fn prepare(
        &self,
        server: &McpServer,
        permissions: &[PersistedPermission],
        profiles_dir: &std::path::Path,
    ) -> AppResult<SandboxedCommand> {
        let profile = build_macos_profile(permissions);
        let profile_path = profiles_dir.join(format!("{}.sb", server.id));
        std::fs::create_dir_all(profiles_dir)?;
        std::fs::write(&profile_path, profile)?;

        let mut args = vec![
            "-f".to_string(),
            profile_path.to_string_lossy().into_owned(),
            server.command.clone(),
        ];
        args.extend(server.args.clone());

        Ok(SandboxedCommand {
            program: "sandbox-exec".to_string(),
            args,
            enforcement: "macos-sandbox-exec",
        })
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub struct MacosSandbox;

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
impl Sandbox for MacosSandbox {
    fn prepare(
        &self,
        server: &McpServer,
        _permissions: &[PersistedPermission],
        _profiles_dir: &std::path::Path,
    ) -> AppResult<SandboxedCommand> {
        // Fallback for code that explicitly constructs MacosSandbox on
        // a non-macOS target (rare; kept for trait-object completeness).
        Ok(SandboxedCommand {
            program: server.command.clone(),
            args: server.args.clone(),
            enforcement: "noop",
        })
    }
}

// ---------------------------------------------------------------------------
// Sandbox profile generation (Apple's SBPL — see `man sandbox-exec`).
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn build_macos_profile(permissions: &[PersistedPermission]) -> String {
    // Strategy: **permissive baseline + explicit denies**.
    //
    // A strict default-deny profile is the textbook approach, but Node /
    // npm / npx need so many syscalls to even start (dyld cache reads,
    // mach bootstrap, /etc lookups, sysctls, …) that maintaining a
    // working allow-list is brittle — a missing rule kills the child
    // before it prints a single byte. For MVP the demo narrative we care
    // about is *network restriction* and *file-write scope*, so we
    // explicitly deny those and let the rest run.
    //
    // The SBPL precedence rule we rely on: the *last* matching rule
    // wins. So `(allow default)` then `(deny network*)` results in
    // network being denied while everything else stays allowed.

    let mut p = String::with_capacity(2048);
    p.push_str("(version 1)\n");
    p.push_str(";; Generated by MCP Hub. Permissive baseline + explicit denies.\n");
    p.push_str(";; Reorder/strip rules at your own risk — they encode the\n");
    p.push_str(";; trust contract from the install dialog.\n\n");
    p.push_str("(allow default)\n\n");

    // ---- Gather granted scopes ---------------------------------------------
    let mut allow_network = false;
    let mut net_hosts: Vec<String> = Vec::new();
    let mut fs_write_paths: Vec<String> = Vec::new();

    for perm in permissions.iter().filter(|p| p.granted) {
        match perm.scope.as_str() {
            "fs.write" => {
                if let Some(target) = expand_target(perm.target.as_deref()) {
                    fs_write_paths.push(target);
                }
            }
            "internet" | "net.outbound" => {
                allow_network = true;
                if let Some(host) = perm.target.as_deref() {
                    if !host.is_empty() && host != "*" {
                        net_hosts.push(host.to_string());
                    }
                }
            }
            // fs.read intentionally not enforced at the SBPL layer in this
            // permissive baseline — narrowing reads breaks too many node
            // code paths in practice. Reads stay surfaced in the UI; a
            // tighter profile is on the roadmap.
            _ => {}
        }
    }

    // ---- File-write policy --------------------------------------------------
    p.push_str(";; File-write policy: deny everywhere, then re-allow the\n");
    p.push_str(";; paths the user granted plus the runtime scratch areas.\n");
    p.push_str("(deny file-write*)\n");

    // Runtime scratch — npm / node refuse to function without these.
    write_subpath_write(&mut p, "/private/tmp");
    write_subpath_write(&mut p, "/tmp");
    write_subpath_write(&mut p, "/private/var/folders");
    if let Ok(tmp) = std::env::var("TMPDIR") {
        write_subpath_write(&mut p, tmp.trim_end_matches('/'));
    }
    // npm / nvm / package-manager caches live under the user's home.
    if let Some(home) = dirs::home_dir() {
        let h = home.display().to_string();
        write_subpath_write(&mut p, &format!("{}/.npm", h));
        write_subpath_write(&mut p, &format!("{}/.cache", h));
        write_subpath_write(&mut p, &format!("{}/.nvm", h));
        write_subpath_write(&mut p, &format!("{}/.volta", h));
        write_subpath_write(&mut p, &format!("{}/.bun", h));
        write_subpath_write(&mut p, &format!("{}/.config", h));
    }
    // User-granted scopes.
    for path in &fs_write_paths {
        p.push_str(&format!(
            ";;   granted fs.write scope: {}\n",
            sanitize_for_comment(path)
        ));
        write_subpath_write(&mut p, path);
    }

    // ---- Network policy -----------------------------------------------------
    p.push_str("\n;; Network policy.\n");
    if allow_network {
        if net_hosts.is_empty() {
            p.push_str("(allow network*)\n");
        } else {
            // SBPL `network-outbound` filters on IP, not hostname. We can
            // surface granted hosts as comments for transparency but the
            // actual rule has to be broader. Future hardening: do DNS
            // resolution + emit IP rules, or proxy through localhost.
            for host in &net_hosts {
                p.push_str(&format!(
                    ";;   granted host: {}\n",
                    sanitize_for_comment(host)
                ));
            }
            p.push_str("(allow network*)\n");
        }
    } else {
        p.push_str("(deny network*)\n");
    }

    p
}

#[cfg(target_os = "macos")]
fn write_subpath_write(p: &mut String, path: &str) {
    p.push_str(&format!(
        "(allow file-write* (subpath \"{}\"))\n",
        sanitize_path(path)
    ));
}

#[cfg(target_os = "macos")]
fn expand_target(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    // Resolve `$HOME`, `~/...`, and `*` (which becomes home as the broadest
    // safe interpretation — the SBPL parser would reject a literal `*`).
    let home = dirs::home_dir()?;
    let home_str = home.to_string_lossy().into_owned();
    let resolved = match raw {
        "*" => home_str,
        "~" => home_str,
        s if s.starts_with("~/") => format!("{}/{}", home_str, &s[2..]),
        s if s == "$HOME" => home_str,
        s if s.starts_with("$HOME/") => format!("{}/{}", home_str, &s[6..]),
        s if s.starts_with("${HOME}") => format!("{}{}", home_str, &s[7..]),
        s => s.to_string(),
    };
    Some(resolved)
}

#[cfg(target_os = "macos")]
fn sanitize_path(path: &str) -> String {
    // Escape backslashes and double-quotes so the SBPL parser is happy.
    path.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn sanitize_for_comment(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use chrono::Utc;

    fn perm(scope: &str, target: Option<&str>) -> PersistedPermission {
        PersistedPermission {
            id: 0,
            server_id: "test".into(),
            scope: scope.into(),
            target: target.map(String::from),
            reason: None,
            granted: true,
            granted_at: Some(Utc::now()),
        }
    }

    #[test]
    fn baseline_is_permissive_with_explicit_denies() {
        let p = build_macos_profile(&[]);
        assert!(p.contains("(allow default)"));
        assert!(p.contains("(deny file-write*)"));
        assert!(p.contains("(deny network*)")); // no `internet` grant
    }

    #[test]
    fn fs_write_grants_become_subpath_allows() {
        let perms = vec![perm("fs.write", Some("$HOME/Projects"))];
        let p = build_macos_profile(&perms);
        let home = dirs::home_dir().unwrap().to_string_lossy().into_owned();
        assert!(p.contains(&format!(
            "(allow file-write* (subpath \"{}/Projects\"))",
            home
        )));
    }

    #[test]
    fn internet_grant_replaces_deny_network() {
        let perms = vec![perm("internet", Some("api.github.com"))];
        let p = build_macos_profile(&perms);
        assert!(p.contains("(allow network*)"));
        assert!(!p.contains("(deny network*)"));
    }

    #[test]
    fn revoked_perms_are_skipped() {
        let mut perms = vec![perm("fs.write", Some("/Users/x/x"))];
        perms[0].granted = false;
        let p = build_macos_profile(&perms);
        assert!(!p.contains("(allow file-write* (subpath \"/Users/x/x\"))"));
    }
}
