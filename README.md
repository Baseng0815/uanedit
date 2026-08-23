# uanedit

An open-source editor for OPC UA nodesets.

An OPC UA information model lives in a `.NodeSet2.xml` file. Editing one today
means either a Windows desktop application or a text editor and a copy of the
specification. uanedit is a self-hostable web app that sits in the middle: it
understands the address space, it validates against the spec, and it writes
files clean enough to keep in version control.

**Status: scaffolding only.** The dev shell, the workspace layout, and the
lint/format configuration are in place. No source files exist yet — see
`design/` for the decisions made so far and the questions still open.

## Layout

```
uanedit/
├── flake.nix        nix dev shell: nightly rust + wasm32 target, dioxus-cli
├── Cargo.toml       workspace root, shared deps and lints
├── design/          design decisions, as markdown
├── uanedit/         address space model, NodeSet2 codec, edit operations
└── web/             dioxus-fullstack app: server + browser client
```

`uanedit` is a pure domain crate — no filesystem, no HTTP, no async runtime —
and builds for `wasm32-unknown-unknown`, because `web` compiles it into both
halves. That is what lets server functions take and return domain types with no
DTO layer. File access lives in `web/src/server/`.

The XML codec sits behind a default-off `xml` feature, so the browser bundle
gets the model and the edit operations without the parser.

### Package naming

Directories and code-level names are `uanedit` and `web`; the Cargo package
names are `uanedit` and `uanedit-web`. Dependents alias the packages back, so
code reads `use uanedit::…`.

## Development

The dev shell provides the nightly toolchain (with the `wasm32-unknown-unknown`
target), `dx` (dioxus-cli 0.7.9), `xmllint`, and the LLVM tools:

```sh
nix develop          # or: direnv allow, once
```

Run the app — `dx` builds both halves and hot-reloads:

```sh
cd web && dx serve
```

Checks:

```sh
cargo check --workspace
cargo check -p uanedit-web --no-default-features --features server   # server half
cargo check -p uanedit-web --target wasm32-unknown-unknown           # browser half
cargo clippy --workspace --all-targets
cargo fmt --all                                                      # nightly only
```

`rustfmt.toml` uses nightly-only options, so format with the nightly toolchain —
the dev shell's default already is. Under rustup, prefix `+nightly`.

### Workspace directory

There is no database. The server reads and writes nodeset files under a
workspace directory, `UANEDIT_WORKSPACE`, defaulting to `./workspace` (which is
gitignored). Point it at wherever your models live.

## What matters most

A file loaded and saved without edits must come back out byte-identical.
Nodesets belong in version control, and a save that reflows the document buries
the one real change under thousands of lines of noise. Everything about the
model — ordered maps, preserved lexical forms, preserved unknown elements —
follows from that. See `design/general/architecture.md`.

## Next steps

In rough dependency order:

1. `uanedit`'s lexical types — `NodeId`, `QualifiedName`, `LocalizedText`.
   Every other type is denominated in them.
2. The node classes and their attributes (OPC 10000-3), and the `NodeSet`
   container: namespaces, models, aliases, nodes.
3. The NodeSet2 reader and writer (OPC 10000-6 Annex F), with round-trip tests
   over the standard nodeset and a companion spec.
4. The web shell: Material 3 tokens, navigation, and the tree/inspector panes.
5. Validation, surfaced in the UI rather than blocking saves.

## License

MIT OR Apache-2.0.
