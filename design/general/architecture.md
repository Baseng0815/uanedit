# Architecture

Status: decided at project setup (2026-08-23), before any code; implemented as
of 2026-08-24. The decisions below all held. Where building one taught us
something the decision did not say, an "as built" note records it.

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

**As built.** The consequences above were not enough on their own: a writer
that re-serialises a faithful model still normalises whitespace, attribute
order and self-closing tags. What the invariant actually needed is a **splicing
writer**. `xml::Document` keeps the source bytes beside the model, plus a
`Layout` recording the byte span of every table and every node. Writing walks
those regions, re-reads each one, and emits the original bytes wherever the
source still says what the model says — re-serialising only the regions that
disagree. An edit therefore rewrites one start tag, and an unedited file is
returned rather than reproduced.

Nothing has to tell the document that an edit happened. `write_nodeset` takes a
whole `NodeSet` — including one that was serialised to the browser, edited
there, and sent back — and finds the difference by comparison. That is what
lets the browser own the editing (§4) without an edit journal on the wire.

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

**As built.** Editing is client-authoritative. `open_file` sends the parsed
nodeset, its resolved dependencies, the open report and the acknowledgements;
the browser builds the `AddressSpace` and the `Session` from that payload and
applies every operation locally, so no edit is a round trip and undo/redo needs
no server at all. The server keeps only the `Document` — the bytes and the
layout — in a registry keyed by file name, and a save is the whole edited
`NodeSet` coming back to be spliced into those bytes (§3).

Acknowledgements are the second thing on disk beside the nodeset: a
`<file>.acks.json` sidecar, written next to the file it annotates so it is
committed with the model (guardrails.md §4, §8.1).

## 5. Namespace 0 is read-only

Nodes in `http://opcfoundation.org/UA/` are defined by the standard. The editor
shows them as reference targets and type parents and never lets them be
edited. Resolving them needs the standard nodeset available to the server,
which §7.1 settles: it is a workspace file like any other.

A small built-in fallback exists regardless — the standard ReferenceType
hierarchy and the handful of NodeIds the model itself names
(`uanedit/src/space/standard.rs`, `uanedit/src/ids.rs`) — so the engine can
answer hierarchy questions and a picker has something to offer before
`Opc.Ua.NodeSet2.xml` is loaded. A loaded namespace 0 states the same relations
and merges with them.

## 6. Validation

Three tiers, worth keeping distinct because they have different severities:

1. **Schema** — does the file match the UANodeSet XSD.
2. **Structural** — dangling reference targets, duplicate `NodeId`s, browse
   names colliding among siblings, a namespace index with no table entry.
3. **Conformance** — rules from OPC 10000-3/5 that the schema cannot express:
   a variable with no `HasTypeDefinition`, an abstract type instantiated, a
   mandatory child missing from an instance of a type.

Findings never gate saving — a nodeset under construction is invalid most of
the time, and an editor that refuses to save one is not an editor. Strictness
lives one level down, at the edit operations: the editor's own operations
refuse to introduce new errors, findings inherited from imported files can be
acknowledged, and an explicit override exists for expert edits. The full
stance, including the single rules engine behind operations, pickers, and
diagnostics, is `guardrails.md`.

## 7. Open questions

1. **Standard nodesets.** *Resolved 2026-08-24: the user places
   `Opc.Ua.NodeSet2.xml` in the workspace, and it resolves from there like
   every other dependency.* Namespace 0 is treated as a `RequiredModel` of
   every file whether or not the file says so, and is looked up by ModelUri
   among the workspace's siblings; a companion spec is found the same way. The
   binary vendors nothing and fetches nothing, which keeps it small and keeps
   it working offline. A file opened without namespace 0 present opens anyway,
   and the payload carries the fact so the UI can say so.
2. **Live server import.** Should uanedit connect to a running OPC UA server
   and read its address space into a nodeset? Genuinely useful, and a large
   dependency (an OPC UA client stack, TLS, session handling) that would not
   otherwise exist. Deferred, not rejected. **Still open.**
3. **Concurrency.** *Resolved 2026-08-24: single user, and the code says so.*
   The open-document registry is one `Mutex<HashMap<String, OpenDocument>>` in
   one process. Opening a file that is already open returns the held state
   rather than re-parsing it; the lock is held across a parse, so the second of
   two concurrent opens waits and then finds it open. Two tabs editing the same
   file still have no conflict story — last save wins — and that is accepted
   for now rather than solved.
4. **Generated code.** Nodeset editors are often paired with code generation
   (C#/Rust types from a model). Out of scope, but the model should not
   preclude it. **Still open.**
