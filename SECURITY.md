# Security Policy

MCP Hub is **pre-alpha**. The architecture intentionally sits between AI clients and
MCP servers — the trust contract is load-bearing. We take security reports seriously
even at this stage.

## Reporting a Vulnerability

**Do not file a public issue.**

Use GitHub's private vulnerability reporting:

1. Open https://github.com/Dembayev/MCP-HUB/security/advisories
2. Click **Report a vulnerability**
3. Describe the issue, reproduction steps, and (if known) impact / suggested fix

We aim to acknowledge within **48 hours** and provide a fix or mitigation timeline
within **7 days** of acknowledgment. CVE assignment, if applicable, happens after
the fix lands on `main`.

## Supported Versions

Only the `main` branch is supported during the pre-alpha period. Once we tag
`v0.1.0`, supported versions will be tracked here explicitly.

## Threat Model — In Scope

MCP Hub's security model rests on three layers, each of which is in scope for
vulnerability reports:

- **Sandbox enforcement.** Bugs that let an MCP server bypass the platform sandbox
  (macOS `sandbox-exec` profile, Linux AppArmor/seccomp once added, Windows job
  objects once added) and read or write outside its granted scope.
- **Approval flow integrity.** Bugs that let a `tools/call` proceed without the
  user's runtime approval when the required scope isn't already granted — e.g.
  race conditions, channel exhaustion, or modal dismissal bypasses.
- **Audit trail integrity.** Bugs that let a session trace omit, reorder, or
  tamper with action records — e.g. writes that bypass the NDJSON path, or
  `payload_hash` collisions that would let a tampered payload pass verification.

Out of scope (for now, by design — see `docs/SESSION_SCHEMA.md` §12 and the
`mcp_hub_launch_guardrails` of the project):

- Cloud sync, remote logging, multi-user permission models, signed-release
  verification, kernel-level container isolation. These are explicitly future
  work and reports against their absence will be closed as "won't fix (yet)".

## Local-First Posture

MCP Hub stores everything on the user's machine: SQLite registry, session traces,
sandbox profiles. No telemetry. No cloud. Reports that hinge on data leaving the
machine via unintended channels (DNS, analytics SDKs, error reporters) are
high-priority.

## Coordinated Disclosure

We prefer coordinated disclosure: please give us time to fix and ship before
public discussion. We will credit reporters in the relevant release notes
unless you ask to remain anonymous.
