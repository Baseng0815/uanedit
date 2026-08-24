# Visual direction — Material 3

Status: direction agreed at project setup (2026-08-23); built 2026-08-24. The
token values below survived contact with the screen unchanged and are the ones
in `web/assets/main.css`, which carries the rest of the scale — state layers,
elevation, motion, type, density — beside them. §5 records what building them
settled.

## 1. Decisions

| Question       | Decision                                                          |
| -------------- | ----------------------------------------------------------------- |
| Design system  | Material 3, hand-written                                           |
| Implementation | Custom properties in `web/assets/main.css`; no component library    |
| Themes         | Light + dark via `light-dark()` on `color-scheme: light dark`, following the system by default. *Revised 2026-08-24:* an in-app selector after all — an app-bar button cycling system → light → dark, persisted in `localStorage` and applied as a `data-theme` attribute that overrides `color-scheme` |
| Type           | Roboto (variable), self-hosted                                      |
| Icons          | Material Symbols Rounded (variable), self-hosted                    |

Hand-written rather than a Rust Material component crate: the editor's main
screen is a dense tree plus an attribute inspector, which is not what a generic
component library is shaped for, and dioxus's Material offerings are thin. M3
gives the vocabulary — surfaces, tonal roles, elevation, shape scale, motion —
without a dependency that we would spend more time fighting than using.

Self-hosted fonts rather than Google's CDN: the app is self-hostable and has to
work on a machine with no internet, which is the normal condition on a plant
network.

## 2. Color

Seed toward blue. It reads as tooling rather than as a consumer app, and it
leaves green and amber free to carry meaning — which matters here, because
validation severity (§4) is the one place color is load-bearing.

Starting roles, M3 light/dark pairs:

```css
:root {
    color-scheme: light dark;

    --primary:                 light-dark(#00639b, #96ccff);
    --on-primary:              light-dark(#ffffff, #003353);
    --primary-container:       light-dark(#cde5ff, #004a76);
    --on-primary-container:    light-dark(#001d31, #cde5ff);

    --secondary:               light-dark(#51606f, #b8c8da);
    --on-secondary:            light-dark(#ffffff, #233240);
    --secondary-container:     light-dark(#d4e4f6, #3a4857);
    --on-secondary-container:  light-dark(#0d1d2a, #d4e4f6);

    --error:                   light-dark(#ba1a1a, #ffb4ab);
    --error-container:         light-dark(#ffdad6, #93000a);
    --warning:                 light-dark(#7a5900, #e9c349);
    --warning-container:       light-dark(#ffdf9e, #5c4300);

    --surface:                 light-dark(#f7f9ff, #101418);
    --surface-container-low:   light-dark(#f1f4fa, #181c20);
    --surface-container:       light-dark(#ebeef4, #1c2024);
    --surface-container-high:  light-dark(#e6e8ee, #272a2e);
    --on-surface:              light-dark(#181c20, #e0e2e9);
    --on-surface-variant:      light-dark(#42474e, #c2c7cf);
    --outline:                 light-dark(#72777f, #8c9199);
    --outline-variant:         light-dark(#c2c7cf, #42474e);
}
```

Plus M3's shape scale (4/8/12/16/28 px), the standard easing curves, and the
three elevation shadows.

## 3. Layout

The editor is a three-pane workspace — address space tree, node inspector,
validation/references — not a page-per-route app. That is the main way this
differs from an ordinary M3 layout, and the main thing to design properly
before writing CSS.

Provisional: M3 navigation rail on the left for workspace-level destinations
(files, validation, settings), top app bar carrying the open file name and the
save state, panes below. Nothing about this is settled.

**As built.** Exactly that. The rail holds Files, with Validation and Settings
present and disabled — validation lives in the editor's right pane, so the
destination is a placeholder rather than a duplicate. The app bar carries the
brand, the open file with a dirty dot, a transient status message, the
undo / redo / diff / download / save actions, and the theme selector. Download
exists because the editor is deployed on the web where the workspace directory
is the server's: it renders the XML a save would write — unsaved edits
included, nothing touching disk — and hands it to the browser as a file. The three panes are address-space tree,
inspector, and a tabbed right pane (References, Validation); the open-file
report, the diff preview, the version nudge and the wizards are dialogs over
them rather than a fourth pane.

## 4. Color as meaning

The one rule to hold: **blue is interactive, everything else is semantic.**

- `--primary` — links, active nav, primary buttons. Never a status.
- `--error` — validation errors, unresolvable references.
- `--warning` — spec deviations that still save.
- Node classes in the tree are distinguished by icon and shape first. If they
  eventually need color, it comes from a separate ramp, not from the primary
  role — otherwise "is this selected or is it an ObjectType" stops being
  answerable at a glance.

Monospace and `font-variant-numeric: tabular-nums` for `NodeId`s and numeric
attributes; they are scanned in columns.

## 5. Open questions

1. The three-pane layout above the tablet breakpoint, and what it collapses to
   below it. A tree/inspector split has no obvious phone form.
   *Resolved 2026-08-24: two breakpoints, and no phone form is attempted.*
   Below 1100 px the right pane is dropped and the tree narrows; below 760 px
   the remaining two stack into one scrolling column. The editor is a desktop
   tool and degrades rather than reflowing into something unusable. What a
   narrow viewport does about the references and validation the dropped pane
   held is not answered — it is hidden, not relocated.

2. Whether the tree is virtualised from the start.
   *Resolved 2026-08-24: yes, virtualised, and never built any other way.*
   Rows are a fixed 26 px (`--row-height-dense`), which is what makes the
   arithmetic
   possible: scroll offset divided by row height gives the first visible row,
   ten rows of overscan on each side, and only that window is rendered. The
   fixed height is a real constraint on styling — a row cannot grow to fit its
   content, so anything that does not fit is elided or moved to the inspector —
   and it is worth it: the standard nodeset is tens of thousands of nodes and
   flattening the whole graph into DOM is not an option.

3. Icon set for the eight node classes — Material Symbols has no OPC UA
   vocabulary, so these are analogies that need choosing deliberately.
   *Chosen 2026-08-24* (`web/src/views/editor/icons.rs`), for distinct
   silhouettes at 18 px and for pairing an instance class with the class that
   types it:

   | Class         | Symbol            | Reading                                                |
   | ------------- | ----------------- | ------------------------------------------------------ |
   | Object        | `deployed_code`   | a solid box — a thing that exists in the address space  |
   | ObjectType    | `category`        | the classifier those things are sorted into            |
   | Variable      | `label`           | a tag: a named value hung on something                 |
   | VariableType  | `sell`            | the same tag drawn hollow — the template of a value     |
   | Method        | `function`        | `f(x)`: the one node class you call                    |
   | View          | `visibility`      | a chosen way of looking at the graph                   |
   | DataType      | `data_object`     | `{ }`: the shape a value takes                         |
   | ReferenceType | `arrow_right_alt` | the arrow itself                                       |

4. Density. *Resolved 2026-08-24: a compact override on the tokens, as
   expected, and no component fought.* The scale in `main.css`: 52 px app bar,
   76 px navigation rail, 36 px pane headers, 40 px ordinary rows, 26 px tree
   and table rows, and a 4/8/12/16/24 px gap scale. Body text is 0.875 rem,
   small body 0.75 rem. Every M3 component here is written against those
   custom properties, so density is one block to change rather than a sweep.
