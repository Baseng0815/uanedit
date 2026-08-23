# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Status: scaffolding only.** The dev shell, the workspace, and the lint/format
configuration are in place. Neither crate has source files yet, so the build
commands below do not work until `uanedit/src/lib.rs` and `web/src/main.rs`
exist. The Architecture section describes the intended shape, not code you can
read.

## Code guidelines

1. Prefer small, descriptive functions.
2. Add as few comments as possible. At most a single sentence per struct or function, and only if it is ABSOLUTELY necessary — do not add comments that are not extremely useful. This applies to doc comments too: the workspace sets `missing_docs = "warn"`, but do not add doc comments just to silence it.
3. Do not add any unit or integration tests unless specifically asked to. The one standing exception is round-trip tests (see below) — propose them, do not add them unasked.
4. Delegate liberally to subagents: fan out searches, research, and independent pieces of work to parallel agents instead of doing everything in the main context.
5. Never guess at OPC UA semantics. The specification is searchable from this session — see "OPC UA reference material".

## Commands

Development happens inside the Nix dev shell (`nix develop`, or `direnv allow` once), which provides the nightly toolchain with the `wasm32-unknown-unknown` target and `dx` (dioxus-cli 0.7.9).

- Run the app (builds both halves, hot-reloads): `cd web && dx serve`
- Type-check: `cargo check --workspace`
- Check the server half of `web`: `cargo check -p uanedit-web --no-default-features --features server`
- Check the browser half of `web`: `cargo check -p uanedit-web --target wasm32-unknown-unknown` — the check that catches a non-wasm dependency leaking into the bundle; `cargo check --workspace` compiles the `web` feature natively and will not catch it
- Lint: `cargo clippy --workspace --all-targets`
- Format: `cargo fmt --all` — must run on nightly (`rustfmt.toml` uses nightly-only options; stable silently skips them and reports spurious diffs). Under rustup: `cargo +nightly fmt --all`.
- Test one crate: `cargo test -p uanedit`; a single test: `cargo test -p uanedit <test_name>`
- Validate a written file against the schema, independently of our own writer: `xmllint --noout --schema UANodeSet.xsd <file>` (`xmllint` comes from the dev shell)

The `web` package is named `uanedit-web` so it never shadows a registry crate; the domain package is plain `uanedit`. Dependents alias them back, so imports read `use uanedit::…`.

## Architecture

Two workspace crates.

- `uanedit/` — the domain: the OPC UA address space model, the NodeSet2 XML codec, the editing operations, and validation.
- `web/` — dioxus-fullstack app: server and browser client in one crate.

`uanedit` is a pure domain crate — no filesystem, no HTTP, no async runtime — and must build for `wasm32-unknown-unknown`, because `web` compiles it into both halves. File access lives in `web/src/server/`.

The XML codec sits behind a default-off-for-dependents `xml` feature (`quick-xml`). The workspace dependency entry leaves it off so the browser bundle gets the model and the edit operations only; `web`'s `server` feature turns it back on for the native side that reads and writes files. Anything added to `uanedit` that cannot compile to wasm belongs behind that feature.

`web` is compiled twice: to `wasm32-unknown-unknown` for the browser (`web` feature, default) and natively for the server (`server` feature). Both halves depend on `uanedit`, so server functions in `web/src/api/` take and return domain types directly — there is no DTO layer. Every type that crosses the wire therefore needs `Serialize` and `Deserialize`.

Intended `web/src` layout, following the same split:

| `web/src` path | Compiled into       | Holds                                          |
| -------------- | ------------------- | ---------------------------------------------- |
| `views/`       | both                | one module per route                           |
| `components/`  | both                | UI shared across views                         |
| `api/`         | both (split bodies) | server functions over domain types — the wire  |
| `server/`      | server only         | workspace root, file I/O, open-document state  |

Server functions return `ServerFnResult<T>` from `dioxus::prelude` — there is no bare `Result<T>` alias. A function's signature is compiled into both halves; only its body is server-only, so server-only imports go inside the body.

There is no database. The server owns a **workspace directory** of `.NodeSet2.xml` files (`UANEDIT_WORKSPACE`, default `./workspace`, gitignored). Opening, saving, and creating a nodeset are file operations, which is what makes the editor's output reviewable in git alongside the rest of a user's project.

## Round-tripping is the core invariant

A nodeset file loaded and saved without edits must come back out unchanged. Users keep these files in version control; a save that reflows the whole document buries the one attribute they actually changed under thousands of lines of noise.

This constrains the model more than anything else in the codebase:

- Keep insertion order everywhere — node order, reference order, the alias table. Use `IndexMap`, never `HashMap`, for anything that is written back out.
- Keep the lexical form where the spec permits a choice: GUID casing, base64 padding, whether a `NodeId` was written with an alias or spelled out.
- Keep what we do not understand. Unknown child elements, `Extensions`, and attributes from a newer schema revision are preserved, not dropped, so a file written by another tool survives a visit to this one.
- Prefer a pull parser and an explicit writer over serde derives — derives flatten exactly the ordering and unknown-element information this invariant depends on.

Round-trip tests over real nodesets (the standard `Opc.Ua.NodeSet2.xml`, a companion spec, a hand-written file) are the exception to "no tests unless asked".

## OPC UA reference material

An MCP server for the OPC Foundation specifications is available in this session. Use it instead of recalling spec details from memory:

- `search_nodes` — resolve a `NodeId` or `BrowseName` (e.g. `HasComponent` → `nsu=http://opcfoundation.org/UA/;i=47`).
- `search_terms` — find where a term is normatively defined, across specs.
- `search_text` — full-text search inside one document; requires a `docNumber`.
- `search_cu` — conformance units.

The documents that matter here:

| Document       | Covers                                                                 |
| -------------- | ---------------------------------------------------------------------- |
| OPC 10000-3    | Address Space Model — node classes, attributes, `NodeId`, `QualifiedName`, `LocalizedText` |
| OPC 10000-5    | Information Model — the standard nodes in namespace 0                  |
| OPC 10000-6    | Mappings; **Annex F is the UANodeSet XML schema** — the file format this editor exists to edit |
| OPC 10000-100+ | Companion specifications (DI, ADI, Machinery, …) — the nodesets users import and extend |

Namespace index 0 is always `http://opcfoundation.org/UA/`. Nodes in it are defined by the standard and are never editable; the editor shows them read-only as reference targets.

## Verifying UI work

A Playwright MCP server (`playwright` in `.mcp.json`, headless chromium, nix-provided) is available to every agent in this repo. When implementing or changing UI, don't stop at `cargo check`: run `cd web && dx serve`, then navigate to the page with the Playwright tools and screenshot it to confirm the change actually renders as intended.

## Design decisions

Design decisions are persisted as markdown files in `design/`, split into `design/ui/` (screens, layout, UI requirements), `design/workflow/` (how a user moves through a feature), and `design/general/` (cross-cutting architecture decisions). Consult them before designing or building features they cover, and keep new work consistent with them.

The visual direction is Material 3 — see `design/ui/material.md`. Tokens are hand-written custom properties in `web/assets/main.css`; there is no component library.
