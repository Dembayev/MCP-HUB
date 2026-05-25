<!--
Thanks for the PR. A few notes before opening:

- One focused change per PR. If you find yourself wanting "AND I also
  cleaned up X", split that out into its own PR.
- New features that introduce schema-touching changes must update
  `docs/SESSION_SCHEMA.md` AND bump `schema_version` per §7 rules.
- The wire format is frozen at v0.1.0. New questions / interpretation
  gaps go into §12 (open questions), not into the spec body.
-->

## What

<!-- One sentence: what does this change. -->

## Why

<!-- Motivation. Link to issue / discussion if applicable. -->

## How

<!-- Brief technical summary — the design choices that matter. -->

## Verification

- [ ] `cargo fmt --check`
- [ ] `cargo build --lib`
- [ ] `cargo test --lib`
- [ ] `cargo test --tests`
- [ ] `cargo clippy -- -D warnings`
- [ ] `npx tsc --noEmit`
- [ ] Manually exercised the affected UI path in `tauri dev`

## Scope check

- [ ] No new architectural layers (no policy engine, no heuristic risk
      classifier, no cloud sync, no multi-user). See SECURITY.md and
      the project guardrails.
- [ ] Session-trace wire format unchanged OR the change is documented
      in §12 of `docs/SESSION_SCHEMA.md` AND adds a version-aware
      compatibility path.
