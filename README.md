# MCP Hub

![status](https://img.shields.io/badge/status-pre--alpha-orange) ![license](https://img.shields.io/badge/license-MIT-blue)

> **Docker Desktop for MCP servers and AI agents.** Local-first, secure, and beautiful.

> ⚠️ **Pre-alpha — building in public.** The core architecture is in place but the app is not yet installable. Star the repo to watch progress toward v0.1 alpha. Issues / bug reports are not yet useful — open a [Discussion](../../discussions) instead.

<p align="center">
  <img src="screenshots/timeline-denial.png" alt="MCP Hub Timeline showing an AI agent's denied attempt to write to ~/.ssh/config" />
</p>
<p align="center">
  <sub>Timeline — every action your AI agents took, with sandbox decisions surfaced inline. Red glow = denied.</sub>
</p>

MCP Hub is an open-source desktop app that turns the chaotic world of [Model Context Protocol](https://modelcontextprotocol.io/) servers into a one-click experience. Install servers from a marketplace, run them in isolated processes, review what they can access, and watch them work — all from a single window. No cloud, no telemetry, no Electron.

<p align="center">
  <em>Built with Tauri 2 · Rust · React · TypeScript · Tailwind · shadcn/ui</em>
</p>

---

## Why this exists

The MCP ecosystem is growing fast, but the developer experience is rough:

- Setting up a single server means hand-editing JSON configs across multiple clients.
- There's no shared GUI to start, stop, or inspect what's running.
- Permissions are implicit — a server you install can read anything your shell can.
- Discovery is "Google a GitHub repo and hope for a README."
- There's no trust layer.

MCP Hub closes all of these in one place: a fast, beautiful, local-first desktop app that treats MCP servers the way Docker Desktop treats containers.

## What's inside the MVP

- **One-click install** of MCP servers from a curated marketplace
- **Process supervision** — start, stop, restart, view status at a glance
- **Permissions panel** — see and revoke filesystem, network, and exec access per server
- **Live activity & logs** — tail stdout/stderr in real time
- **Local-first storage** — SQLite, encrypted-at-rest, never leaves your machine
- **Sandbox abstraction** — pluggable runner (raw process today; sandbox-exec / job objects / containers next)

## Tech stack

| Layer       | Choice                                                  | Why                                                  |
| ----------- | ------------------------------------------------------- | ---------------------------------------------------- |
| Shell       | [Tauri 2](https://tauri.app)                            | Native binary, ~10MB, no Chromium bundled            |
| Backend     | Rust (`tokio`, `rusqlite`, `parking_lot`)               | Fast, safe, single binary                            |
| Frontend    | React 18 + TypeScript (strict) + Vite                   | Industry-default DX, instant HMR                     |
| Styling     | Tailwind CSS + shadcn/ui (new-york)                     | Design-system primitives without the runtime cost    |
| Animation   | Framer Motion                                           | Polished micro-interactions where they matter        |
| Persistence | SQLite (bundled, WAL mode)                              | Zero-config local store                              |

## Project layout

```
mcp-hub/
├── src/                      # React frontend
│   ├── components/
│   │   ├── ui/               # shadcn primitives (button, card, badge, input)
│   │   ├── layout/           # app shell, sidebar, titlebar
│   │   └── servers/          # server card, list, empty state, status dot
│   ├── hooks/                # useMcpServers — talks to Tauri or mocks
│   ├── lib/                  # tauri.ts (typed invoke), utils
│   ├── pages/                # one file per route
│   └── types/                # IPC payload types (mirror Rust models)
├── src-tauri/                # Rust backend
│   ├── src/
│   │   ├── commands/         # Tauri commands — thin IPC surface
│   │   ├── db/               # SQLite + migrations + row models
│   │   ├── mcp/              # registry + process manager
│   │   ├── security/         # permissions model + sandbox trait
│   │   ├── error.rs          # AppError + Serialize impl for IPC
│   │   ├── state.rs          # shared AppState
│   │   └── lib.rs            # wiring + tauri::Builder
│   ├── capabilities/         # Tauri 2 permission capabilities
│   ├── icons/                # platform icons
│   └── tauri.conf.json
└── package.json
```

## Getting started

Requirements: **Rust 1.77+**, **Node.js 18+**, and the [Tauri prerequisites](https://tauri.app/v2/guides/getting-started/prerequisites) for your OS.

```bash
# install JS dependencies
npm install

# run the desktop app in dev mode (recompiles Rust on change)
npm run tauri:dev

# build a production binary
npm run tauri:build
```

For frontend-only iteration (faster HMR, mock data — no Tauri shell needed):

```bash
npm run dev
```

`useMcpServers` automatically falls back to a mock dataset when not running inside Tauri, so designers can work on the UI without touching Rust.

## Architecture notes

- **State management** lives in Rust, not React. The frontend is a view layer; all writes go through Tauri commands so the source of truth is one place.
- **Errors are stringified at the IPC boundary.** `AppError` implements `Serialize` as its `Display` form, so the frontend gets clean messages without leaking enum internals.
- **The sandbox is a trait.** `NoopSandbox` ships today; platform-specific implementations (`sandbox-exec` on macOS, AppArmor on Linux, job objects on Windows) plug in behind the same `Sandbox::prepare` signature without touching IPC or UI code.
- **No router.** The app has five top-level routes — a tiny piece of `useState` beats pulling in `react-router`. We'll revisit if/when we need URL deep-linking.
- **Bundle stays lean.** The dependency list is deliberately small: no UI framework runtime beyond React, no state library, no data-fetch lib. Tauri keeps the binary around 10 MB.

## Roadmap

- [x] Scaffold + foundational architecture
- [x] Server registry (install, list, start, stop, remove)
- [ ] Marketplace UI + curated registry feed
- [ ] Live log streaming (stdout/stderr tail)
- [ ] Per-server permission grants with UI prompts
- [ ] Platform sandboxes (`sandbox-exec`, AppArmor, job objects)
- [ ] Encrypted secrets vault for env vars
- [ ] OS-native auto-update
- [ ] Tauri-mobile companion (read-only first)

## Contributing

Issues and PRs welcome. See [CONTRIBUTING.md](./CONTRIBUTING.md) for the dev setup, code style, and how the modules fit together.

## License

[MIT](./LICENSE)
