# Type-aware creation

Status: decided 2026-08-23. The two wizards that make the editor a modeler:
create an instance of a type, and create a subtype. Both are thin UI over
rules-engine queries (general/guardrails.md §1). Rules verified against OPC
10000-3 §6 (InstanceDeclarations, ModellingRules) and §4.6.

## 1. Create instance of type X

1. User picks a type. The picker offers concrete types only — abstract types
   are absent, not greyed out with an error later.
2. The engine returns the type's **fully-inherited instance-declaration
   hierarchy**: declarations collected from all supertypes, identified by
   BrowsePath (not NodeId), nearest override winning.
3. The wizard renders it as a tree of children to materialize:
   - **Mandatory** — pre-checked and locked, materialized recursively.
   - **Optional** — checkboxes, off by default.
   - **OptionalPlaceholder / MandatoryPlaceholder** — a repeatable "add a
     named child of type T" affordance; the user supplies the BrowseName.
     MandatoryPlaceholder requires at least one. A placeholder's own children
     are not considered during instantiation.
   - **ExposesItsArray** and declarations without a modelling rule — skipped.
4. Materialized children keep the declaration's BrowseName verbatim,
   **including its namespace** (a `2:ParameterSet` from a companion spec stays
   `2:ParameterSet` on the user's node). Their HasTypeDefinition points at the
   declaration's type; the user may narrow it to a subtype.
5. NodeIds are auto-assigned in the user's namespace. Copied Methods get
   `MethodDeclarationId` pointing at the type's method node; their argument
   Properties are Mandatory.

Type-definition defaults when creating bare instances outside the wizard:
Objects default to BaseObjectType, DataVariables to BaseDataVariableType;
Properties always point to PropertyType exactly — not a default, a rule.

## 2. Create subtype

Inherited declarations are listed read-only until the user chooses to
override. An override:

- keeps BrowseName and NodeClass — never editable on an override;
- may narrow the type definition to a subtype, never change it sideways;
- may tighten the modelling rule only: Optional → Mandatory,
  OptionalPlaceholder → MandatoryPlaceholder; never loosen. (Exception: a
  placeholder Method concretized by the subtype switches to Optional or
  Mandatory and defines its arguments.)
- materializes its own HasModellingRule and HasTypeDefinition references even
  when unchanged, plus the hierarchical reference linking it in;
- for Variables, follows the attribute-narrowing rules: DataType only to a
  subtype; ValueRank only further restricted; an ArrayDimensions entry only
  from 0 to a fixed value.

The wizard discourages no-op overrides (spec: a subtype should not override a
node unless it changes something).

Method overrides may append optional arguments only, after all inherited ones.

## 3. Standard NodeIds the wizards depend on

These are the constants in `uanedit/src/ids.rs`.

| Node | NodeId |
| --- | --- |
| HasTypeDefinition | i=40 |
| HasModellingRule | i=37 |
| ModellingRuleType | i=77 |
| Mandatory | i=78 |
| Optional | i=80 |
| ExposesItsArray | i=83 |
| OptionalPlaceholder | i=11508 |
| MandatoryPlaceholder | i=11510 |
| BaseObjectType | i=58 |
| BaseDataVariableType | i=63 |
| PropertyType | i=68 |

## 4. Open questions

1. Whether the instance wizard offers "reference the declaration" /
   "reference an existing node" as alternatives to copying — the spec permits
   all three for satisfying a modelling rule. *Partly resolved 2026-08-24:
   copying is what is implemented, and it is the only thing the wizard offers.*
   `Selections` carries optionals to materialise, children named for
   placeholders, and type narrowings — no shape for "point at this instead".
   Copying is what users expect of a modeller and what makes the result
   editable; the other two produce an instance whose children are not its own,
   which needs an inspector story before it needs a wizard. **The other two
   remain open**, and adding them is a change to `Selections`, not to the plan
   the engine returns.
2. Placeholder BrowseName convention. *Resolved 2026-08-24: strip on pre-fill,
   refuse on submit.* `PlannedDeclaration::suggested_browse_name` returns the
   declaration's BrowseName without the angle brackets, keeping the namespace,
   so the field starts at `DeviceParameter` rather than `<DeviceParameter>` and
   the user edits a name rather than deleting punctuation. Submitting a name
   that still contains `<` or `>` is refused — `PlaceholderNameNotConcrete` —
   because a placeholder name written onto a real child would read as a
   declaration to every tool downstream.
