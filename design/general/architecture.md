# Architecture

Status: decided at project setup (2026-08-23), before any code. Everything here
is cheap to revisit while the crates are still empty.

## 1. What this is

uanedit is an editor for OPC UA nodesets — the `.NodeSet2.xml` files that
define an information model. The existing tooling for this is either a Windows
desktop application or a text editor; uanedit is a self-hostable web app that
sits in the middle.

## 2. Crates

| Crate      | Package        | Holds                                                         |
| ---------- | -------------- | ------------------------------------------------------------- |
| `uanedit/` | `uanedit`      | address space model, NodeSet2 codec, edit operations, validation |
| `web/`     | `uanedit-web`  | dioxus-fullstack app — server and browser client               |

`uanedit` is a pure domain crate: no filesystem, no HTTP, no async runtime. It
has to build for `wasm32-unknown-unknown` because `web` compiles it into both
halves, which is what lets server functions take and return domain types with
no DTO layer in between.

The library is deliberately usable without the app. A NodeSet2 model with a
faithful codec is worth publishing on its own, and keeping it honest — no
`web`-shaped shortcuts — is what keeps the editor's core testable.

## 3. Round-tripping

The single invariant everything else bends around: **a file loaded and saved
without edits comes back out unchanged.** Users keep nodesets in version
control. A save that reflows the document buries the one changed attribute in
thousands of lines of diff noise, and that alone would make the editor unusable
for its actual audience.

Consequences, in decreasing order of how easy they are to get wrong:

- Ordered maps (`IndexMap`) for anything written back out — nodes, references,
  the alias table. Never `HashMap`.
- The lexical form is data where the spec permits a choice: GUID casing, base64
  padding, whether a `NodeId` was written as an alias or spelled out.
- Unknown child elements, `Extensions`, and attributes from a newer schema
  revision are preserved rather than dropped.
- A pull parser and an explicit writer (`quick-xml`), not serde derives, which
  flatten exactly the ordering and unknown-element information this depends on.

`xmllint --schema` against the UANodeSet XSD is the independent check that our
writer produces a schema-valid file; it is in the dev shell for that reason.

## 4. Persistence

No database. The server owns a **workspace directory** of nodeset files
(`UANEDIT_WORKSPACE`, default `./workspace`, gitignored). Open, save, and
create are file operations.

This follows from §3: if the point is that the output is reviewable in git,
then the output has to be files in the user's own tree, not rows the user has
to export. It also keeps the deployment story to a single binary and a
directory.

Undo/redo is in-memory, per open document, and lives in `uanedit` as an edit
log over the address space — not in the UI layer, so a headless caller gets it
too.

## 5. Namespace 0 is read-only

Nodes in `http://opcfoundation.org/UA/` are defined by the standard. The editor
shows them as reference targets and type parents and never lets them be
edited. Resolving them needs the standard nodeset available to the server;
whether that is vendored, downloaded, or pointed at by configuration is open
(§7).

## 6. Validation

Three tiers, worth keeping distinct because they have different severities:

1. **Schema** — does the file match the UANodeSet XSD.
2. **Structural** — dangling reference targets, duplicate `NodeId`s, browse
   names colliding among siblings, a namespace index with no table entry.
3. **Conformance** — rules from OPC 10000-3/5 that the schema cannot express:
   a variable with no `HasTypeDefinition`, an abstract type instantiated, a
   mandatory child missing from an instance of a type.

Tier 2 and 3 findings are surfaced, never blocked on. A nodeset under
construction is invalid most of the time; an editor that refuses to save one is
not an editor.

## 7. Open questions

1. **Standard nodesets.** Vendor `Opc.Ua.NodeSet2.xml` (~10 MB) in the repo,
   fetch it at build time, or require the user to place it in the workspace?
   Affects binary size, offline use, and how companion specs get resolved.
2. **Live server import.** Should uanedit connect to a running OPC UA server
   and read its address space into a nodeset? Genuinely useful, and a large
   dependency (an OPC UA client stack, TLS, session handling) that would not
   otherwise exist. Deferred, not rejected.
3. **Concurrency.** Single user assumed. Multiple browser tabs on one workspace
   file currently have no story.
4. **Generated code.** Nodeset editors are often paired with code generation
   (C#/Rust types from a model). Out of scope, but the model should not
   preclude it.
