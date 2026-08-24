# uanedit

A self-hostable web editor for OPC UA nodesets.

An OPC UA information model lives in a `.NodeSet2.xml` file. Editing one today
means either a Windows desktop application or a text editor and a copy of the
specification. uanedit sits in the middle: it understands the address space, it
checks edits against the spec as they are made, and it writes files clean
enough to keep in version control.

<!-- screenshot: the three-pane editor, light and dark -->

## The round-trip guarantee

**A file loaded and saved without edits comes back out byte for byte.** An
edited file differs only where the edit was — one attribute changed rewrites
one start tag and leaves the other four megabytes untouched.

That is not a nicety. Nodesets belong in version control, and a save that
reorders nodes, renumbers namespaces or reflows the document buries the one
real change under thousands of lines of noise. Everything about the model
follows from it: ordered maps everywhere, preserved lexical forms (GUID casing,
base64 padding, whether a `NodeId` was written as an alias or spelled out), and
unknown elements, foreign `Extensions` and newer-schema attributes kept rather
than dropped — so a file another tool wrote survives a visit here.

## Features

**Editing spine.** Workspace file list; create, open, save. Virtualized
address-space tree over the whole graph, with node-class icons, per-namespace
filter chips, and search across BrowseName, DisplayName, NodeId and
SymbolicName. Nodes with no hierarchical parent get an "unrooted" bucket rather
than being silently hidden. Per-class attribute inspector, including the
design-only metadata the file format carries and no runtime ever shows
(SymbolicName, Category, Documentation, ReleaseStatus). References panel with
forward and synthesized-inverse references. Undo/redo over semantic operations.

**Checked like a compiler.** Every structural rule the spec defines is checked
where it would be introduced, not at save time. Findings carry a stable
`UAxxxx` code, a one-line message, an explanation citing the clause it comes
from, and a machine-applicable fix wherever one is computable. Errors that came
in with the file can be **acknowledged** — muted, still counted, re-surfaced if
the facts change — and an explicit **override** performs an edit the engine
would refuse, attributing what it lets through. Saving is never gated.

**Type-aware creation.** Create an instance of a type from its fully inherited
instance-declaration hierarchy: mandatory children materialized, optional ones
offered, placeholders named by the user. Create a subtype, overriding inherited
declarations under the narrowing rules the spec allows.

**Structured editors.** DataTypeDefinition (structures, unions, enumerations,
option sets) and Method `InputArguments` / `OutputArguments`, both constrained
to spec-valid combinations. Typed value editing for scalars and one-dimensional
arrays of built-in types; every other value renders as the XML a save would
write.

**Document surface.** Namespace table, Model entries and RequiredModel
dependencies, alias table. `RequiredModel` entries resolve against sibling
files in the workspace, with namespace-index remapping; dependency namespaces
are browsable read-only and their types instantiable.

**Fidelity made visible.** A unified **diff preview** of exactly what a save
would write, before writing it. An **open-file report** of what was loaded,
what was preserved without being understood, and what was irregular. A
**version-bump nudge** when a save changed the model and its ModelVersion and
PublicationDate did not.

## Quickstart

The Nix dev shell provides everything — the nightly toolchain with the
`wasm32-unknown-unknown` target, `dx` (dioxus-cli 0.7.9), a matching
`wasm-bindgen-cli`, and `xmllint`:

```sh
nix develop          # or: direnv allow, once
cd web && dx serve
```

Nodeset files live in a **workspace directory** — there is no database.
`UANEDIT_WORKSPACE` names it; without it the server uses `./workspace`, falling
back to `../workspace`, which is what makes the repository's own gitignored
`workspace/` the one `dx serve` opens.

Drop your `.NodeSet2.xml` files in there. Add the standard
`Opc.Ua.NodeSet2.xml` too: namespace 0 resolves from the workspace like any
other dependency, and without it the editor can only show the standard nodes it
has built in. Companion specs a model requires (`Opc.Ua.Di.NodeSet2.xml` and
friends) go in the same directory. All of them come from the OPC Foundation's
[UA-Nodeset](https://github.com/OPCFoundation/UA-Nodeset) repository.

## Layout

```
uanedit/
├── flake.nix        nix dev shell: nightly rust + wasm32 target, dioxus-cli
├── Cargo.toml       workspace root, shared deps and lints
├── design/          design decisions, as markdown
├── uanedit/         model, NodeSet2 codec, address space, rules, edit operations
└── web/             dioxus-fullstack app: server + browser client
```

`uanedit` is a pure domain crate — no filesystem, no HTTP, no async runtime —
and builds for `wasm32-unknown-unknown`, because `web` compiles it into both
halves. That is what lets server functions take and return domain types with no
DTO layer, and what lets the browser own the editing: it builds the address
space from what `open_file` sends and applies every operation locally. The
server holds the file's bytes, and a save splices the returned model back into
them. File access lives in `web/src/server/`.

The XML codec sits behind a default-off `xml` feature, so the browser bundle
gets the model and the edit operations without the parser.

Directories and code-level names are `uanedit` and `web`; the Cargo package
names are `uanedit` and `uanedit-web`, so neither shadows a registry crate.
Dependents alias the packages back, so code reads `use uanedit::…`.

## Status and limitations

Working, and not yet 1.0.

- **Single user.** One process, one workspace, one editing session. Two browser
  tabs on the same file have no story.
- **No live server connect.** uanedit edits files; it does not read an address
  space out of a running OPC UA server. Deferred, not rejected.
- **Value editing is at its first tier.** Scalars and one-dimensional arrays of
  built-in types are editable; structures driven by a DataTypeDefinition,
  matrices and Decimal are shown as read-only XML and preserved verbatim.
- **`UANodeSetChanges` files are refused**, politely and by design. The schema's
  second root element is a change set, not a model, and out of scope.
- **Code generation is out of scope**, as is multi-user collaboration.

See `design/` for the decisions behind all of this, and which questions are
still open.

## License

MIT OR Apache-2.0.
