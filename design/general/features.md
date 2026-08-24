# Feature map

Status: decided 2026-08-23, from spec research (OPC 10000-3/5, 10000-6 Annex F,
schema 1.05.04) and a survey of existing tooling (UaModeler, SiOME, CESMII/OPCF
Profile Designer, FreeOpcUa modeler, Sterfive, UA-ModelCompiler). The v1 cut
(§3) shipped 2026-08-24.

## 1. Positioning

Baseline modeling features are commoditized — every serious tool loads
namespace 0 and companion specs read-only, edits all eight node classes, does
modelling-rule-aware instantiation, validates, exports. The evidenced gaps in
the field, in order:

1. **Output stability.** No surveyed tool claims byte-stable round-tripping;
   several reorder, renumber, or reformat on export. Our core invariant is the
   differentiator.
2. **Import robustness.** "File from tool X won't load" is the most common
   failure in the open-source trackers. "Keep what we do not understand"
   addresses it.
3. **Correct type-hierarchy edits.** The leading commercial tool corrupts
   child nodes on base-type changes.
4. **No good free, web-based, file-backed option exists.**

We differentiate on fidelity and strictness ([guardrails](guardrails.md)), not
on ecosystem hooks (code generation, PLC binding, cloud libraries) — those are
the incumbents' moats and out of scope.

## 2. The map

### A. Editing spine

Open → browse → inspect → edit → save.

- Workspace file list; open, create, save; dirty state in the app bar.
- Address-space tree: virtualized (ns 0 alone is ~35 000 nodes), node-class
  icons, namespace filter, search over BrowseName / DisplayName / NodeId /
  SymbolicName. A node under multiple hierarchical parents appears under each;
  nodes with no incoming hierarchical reference are legal and get an
  "unrooted" bucket, never silently hidden.
- Inspector: per-class attribute forms; localized-text lists (one entry per
  locale) for DisplayName / Description / InverseName; the design-only
  metadata the file format carries and no runtime shows (SymbolicName,
  Category, Documentation, ReleaseStatus, DesignToolOnly) — this editor is the
  only UI those fields will ever have. Namespace 0 and dependency nodes render
  read-only.
- References panel: forward and synthesized-inverse references, add / remove /
  retarget, navigate-to-target. Only one direction is ever written to the file.
- Node lifecycle: create (class, parent, reference type, auto-assigned NodeId
  in the user's namespace), delete (ownership-style — see guardrails).
- Undo/redo (edit log in `uanedit`, per architecture.md §4).
- Validation panel fed live by the rules engine; findings navigate to the node.

### B. Modeler features

What separates a modeler from an XML forms app. All are thin UI over rules-
engine queries.

- **Create instance of type X** and **create subtype** — the modelling-rule-
  aware wizards, specified in
  [workflow/type-aware-creation.md](../workflow/type-aware-creation.md).
- **DataTypeDefinition editor**: structure fields (DataType, ValueRank,
  IsOptional, AllowSubTypes, MaxStringLength), unions, enumerations, option
  sets. Only the spec-valid flag combinations are constructible.
- **Method arguments editor**: InputArguments/OutputArguments as `Argument[]`
  Properties. This is a structured value, so it forces a small early slice of
  the value machinery (§D).
- Tier-3 conformance lints (the rule list lives with the engine).

### C. Document-level surface

- Namespace table and Model entry editor: ModelUri, Version, PublicationDate,
  ModelVersion (mandatory since spec 1.05.03), RequiredModel dependencies.
- Version-bump nudge on save: the model changed — bump ModelVersion /
  PublicationDate? (Stale version info in published nodesets is a documented
  ecosystem complaint.)
- Dependency loading: RequiredModels resolve against sibling files in the
  workspace; dependency namespaces are browsable read-only and their types
  instantiable, with namespace-index remapping between files. The workspace
  directory is the universe; namespace 0 is the only special case
  (architecture.md §7.1).
- Alias table: preserved lexically; new references prefer an existing alias.

### D. Values — deliberately tiered

A Variable's Value is a lax-schema XML Variant; structured values are
ExtensionObjects whose bodies follow a per-model generated schema, and
namespace indexes *inside* values point at the file's own tables.

- v1: any value renders as read-only pretty-printed XML (stored verbatim);
  typed editing for scalars and one-dimensional arrays of built-in types only.
- Later: DataTypeDefinition-driven structure editing, Matrix, Decimal,
  per-locale `Translation` elements.

### E. Fidelity made visible

- **Diff preview before save** — we hold the loaded bytes and the would-be
  output; showing the minimal diff is nearly free and is the invariant as a
  feature.
- **Download** *(added 2026-08-24)* — the same would-be output handed to the
  browser as a file, unsaved edits included, nothing written to disk. Exists
  because a web deployment's workspace directory is the server's.
- **Open-file report**: what was loaded, what was preserved-but-unknown
  (foreign extensions, newer-schema attributes), what failed and why. Opaque
  import errors are an unserved complaint across the field.

## 3. v1 cut

A + C + E, with B's wizards as the first follow-up and D at its v1 tier. The
rules engine is not a feature of any tier — it is core, built directly after
the codec, because A's pickers and B's wizards both sit on it.

**Shipped 2026-08-24.** A, C and E in full; B in full, wizards and both
structured editors (DataTypeDefinition, method arguments); D at its v1 tier —
typed editing for scalars and one-dimensional arrays of built-in types, and
read-only XML for everything else, rendered by the same encoder a save uses so
the two can never disagree. What §2D defers stays deferred.

## 4. Out of scope

Live server connect, code generation, multi-user collaboration (architecture.md
§7). `UANodeSetChanges` files (the schema's second root element): detect and
refuse politely.

## 5. Open questions

1. Does the diff preview diff against last save or against git HEAD?
   *Resolved 2026-08-24: against the bytes the server holds — the file as it
   was last read or last written.* That is the comparison the invariant is
   about, it needs no git dependency and no shelling out, and it is free
   because the server already has both sides. Diffing against HEAD would answer
   a different question (what this branch changed), which git itself answers
   better. The diff is computed server-side for the same reason: shipping two
   multi-megabyte documents to the browser to compare them there would cost
   more than the answer.
2. Where search lives in the three-pane layout (ui/material.md §3).
   *Resolved 2026-08-24: in the tree pane, a control row directly under its
   header, above the rows and beside the namespace filter chips.* Search is a
   way of looking at the tree, not a fourth destination, and results replace
   the rows in place. It is debounced, because the needle runs over every node
   in the space.
