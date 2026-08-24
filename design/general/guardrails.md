# Guardrails

Status: decided 2026-08-23. The stance: check like a compiler. Every structural
modeling error the spec defines is caught at the moment it would be
introduced, not at save time — and never silently.

## 1. One rules engine, three consumers

The encoding of the OPC 10000-3/5 constraints lives in `uanedit` as a single
rules engine, consumed three ways:

1. **Operation preconditions.** The crate's public edit API is semantic
   transactions, not graph twiddling: creating a Variable demands a parent,
   reference type, type definition, and data type in one call, so no sequence
   of public calls yields a Variable without a `HasTypeDefinition`. Invalid
   states are unrepresentable in the API wherever the model allows it.
2. **Picker domains.** "Which reference types are legal between these two
   nodes," "what may this DataType narrow to" are queries against the engine.
   The UI never filters a choice list itself, so pickers and checker cannot
   disagree.
3. **Diagnostics.** The validation panel is the same engine in report mode
   over the loaded graph.

Consequence for build order: the engine is the second thing built after the
codec — operations, pickers, wizards, and the validation panel all sit on it.

## 2. Diagnostics

- Every rule has a stable identifier, a one-line message, and an explanation
  citing the spec clause — `UA0305: a Property is the source of a hierarchical
  reference (OPC 10000-3 §5.6.3)` — in the manner of `rustc --explain`.
- Findings carry a machine-applicable fix wherever one is computable (missing
  type definition → suggest the default; missing mandatory child → offer to
  materialize it). A finding without a fix action is the exception.
- Errors are spec violations; warnings are deviations the spec tolerates.
  Operations refuse to introduce errors; warnings are permitted but surface
  immediately.
- Checking is incremental — rules re-evaluate from an edit delta, never by
  whole-graph re-runs. Graph-global rules (HasChild acyclicity, BrowsePath
  uniqueness) are designed for this from the start; this is a day-one
  constraint on the engine, not a later optimization.

**As built (2026-08-24).** The registry is one macro invocation in
`uanedit/src/rules/code.rs`, which defines every code together with its
`UAxxxx` identifier, its default severity, its one-line message and its
explanation. Thirty-seven codes so far, in four bands: `UA01xx` structural,
`UA02xx` references, `UA03xx` instances and modelling rules, `UA04xx`
variables. The rules themselves are in `uanedit/src/rules/checks/`, one module
per band, and a debug assertion in `checks::all()` holds the rule list and the
code list to the same length — a code with no rule behind it fires the moment
the engine is built.

## 3. Operations never introduce findings

The hard guarantee, the editor's analog of "safe Rust has no UB": no operation
offered by the editor produces a new error finding.

- **Type edits are refactorings.** Changing a type that has subtypes or
  instances computes every consequence (DataType may only narrow, ValueRank
  only restrict, modelling rules only tighten, overrides stay consistent) and
  applies atomically or not at all. Never a partial edit.
- **Deletion is ownership-shaped.** Deleting a node never leaves dangling
  references: the operation lists every incoming reference and every attribute
  that names the node (DataType, ParentNodeId, MethodDeclarationId, definition
  fields, role grants) and requires a resolution — retarget, clear or cascade —
  in the same transaction.

## 4. Files we did not write

Nodesets in the wild contain modeling errors — including official companion
specs — and the round-trip invariant forbids fixing them silently. Opening is
never refused; the file is diagnosed, like a compiler on broken source.

Findings therefore split:

- **Inherited** — present in the file as loaded (baselined at open).
- **Introduced** — the result of an edit in this editor. Impossible for
  errors, except through the override (§5).

Inherited findings can be **acknowledged**: the user reviews a finding and
marks it as seen; it collapses to a muted state in the panel — still counted,
never deleted, re-surfaced if the underlying facts change. The analog of
`#[allow]`.

Acknowledgements persist in a committable sidecar in the workspace, never in
the nodeset file itself — writing them there would pollute the diff, and
dependency files are read-only. They are keyed by a fingerprint (diagnostic
code + NodeId + a hash of the finding's salient facts) so they survive
unrelated edits and expire when the finding materially changes.

## 5. The override

`unsafe` for edits: an explicit, per-operation escape hatch that performs an
operation the engine would refuse. Deliberately invoked, visually unmissable,
and it does not suppress the resulting finding — the finding appears in the
panel as introduced-and-acknowledged, attributed to the override. It exists
from v1: a strict checker without an escape hatch refuses legitimate work
(interoperating with broken-but-shipped nodesets) and does not get adopted.

## 6. Saving is never gated

Unchanged from architecture.md §6: a nodeset under construction is invalid
most of the time, and an editor that refuses to save one is not an editor.
The strictness of this document lives at the operations, not at save.

## 7. Limits

"Every structural error" is committed now. Full value-vs-DataTypeDefinition
conformance (an ExtensionObject body matching its structure definition, field
by field) arrives with the value machinery (features.md §2D); the rule list
grows, the architecture does not change.

## 8. Open questions

1. Sidecar format and path for acknowledgements (one file per workspace vs
   per nodeset). *Resolved 2026-08-24: one sidecar per nodeset file, JSON,
   `<file name>.acks.json` beside the file it annotates.* Per nodeset rather
   than per workspace, because an acknowledgement belongs to the model it
   excuses and has to travel with it — into a commit, into a review, into
   another user's checkout. JSON because the sidecar is ours and nothing else
   reads it, so nothing is gained by inventing a format. An absent sidecar is
   an empty set; an unreadable one is an error rather than a silent reset,
   because discarding what the user reviewed would be worse than refusing to
   open the file. Emptying the set removes the file rather than leaving `{}`
   in the tree.
2. Exact fingerprint recipe. *Resolved 2026-08-24:*
   `<code>@<NodeId>#<FNV-1a of the salient facts>`. The code and the NodeId are
   plain text, so a sidecar is readable and greppable; the facts are hashed,
   because they are a rule's own business and vary in length. FNV-1a is written
   out in `uanedit/src/rules/fingerprint.rs` rather than taken from the
   standard library, since an acknowledgement outlives the process that wrote
   it and `DefaultHasher` makes no stability promise.

   Which facts count is the rule's decision, and it is where the balance sits.
   UA0103 (sibling BrowseName collision) is the instructive case: its facts are
   the colliding BrowseName plus the set of NodeIds that share it, **sorted**.
   Sorting means reordering the parent's references does not expire the
   acknowledgement, while a third child joining the collision does. Findings
   that hang off something narrower than a node — a particular reference, a
   named field — fold that detail in ahead of the rule's own facts, so two
   findings of one code on one node stay distinct.
