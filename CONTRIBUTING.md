# Contributing to MCP Hub

Thanks for considering a contribution! MCP Hub is early — the scaffold is in place and we're now adding features. Anything from a typo fix to a new sandbox backend is welcome.

## Dev setup

```bash
# clone and install
git clone https://github.com/<org>/mcp-hub
cd mcp-hub
npm install

# run the app with hot reload (Rust + React)
npm run tauri:dev

# frontend-only mode (mocked data, no Tauri shell)
npm run dev
```

You'll need:

- **Node.js 18+** and **npm**
- **Rust 1.77+** (install via [rustup](https://rustup.rs))
- The [Tauri prerequisites](https://tauri.app/v2/guides/getting-started/prerequisites) for your OS (webview2 on Windows, xcode-select on macOS, libwebkit2gtk on Linux)

## Code style

- **TypeScript**: strict mode is on (`noUncheckedIndexedAccess`, `noUnusedLocals`, etc.). Run `npm run lint` before pushing.
- **Rust**: `cargo fmt` and `cargo clippy --workspace -- -D warnings`.
- **Components**: each component lives in its own file under `src/components/<domain>/`. Co-locate small variants; split when a file passes ~150 lines.
- **No new dependencies without discussion.** One of our goals is a lean bundle; please open an issue first if you want to add a runtime dep.

## Where things live

| You want to…                          | Look at                              |
| ------------------------------------- | ------------------------------------ |
| Add a Tauri command                   | `src-tauri/src/commands/`            |
| Change a DB schema                    | `src-tauri/src/db/mod.rs`            |
| Add a UI page                         | `src/pages/` + register in `App.tsx` |
| Add a shadcn primitive                | `src/components/ui/`                 |
| Tweak the IPC type contract           | both `src/types/mcp.ts` and `src-tauri/src/db/models.rs` |
| Add platform sandboxing               | implement `Sandbox` in `src-tauri/src/security/sandbox.rs` |

## Commit style

Conventional Commits, lowercase:

```
feat(servers): show transport type on card
fix(db): handle missing args column in legacy rows
chore: bump tauri to 2.1.2
```

## Reporting bugs

Please include OS + version, Rust version (`rustc -V`), Node version (`node -v`), and a stack trace or the contents of `~/Library/Application Support/MCP Hub/logs/*.log` (path varies by OS).
