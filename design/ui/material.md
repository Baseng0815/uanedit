# Visual direction — Material 3

Status: direction agreed at project setup (2026-08-23). Nothing is built yet;
the token values below are a starting point to tune on sight, not a decision.

## 1. Decisions

| Question       | Decision                                                          |
| -------------- | ----------------------------------------------------------------- |
| Design system  | Material 3, hand-written                                           |
| Implementation | Custom properties in `web/assets/main.css`; no component library    |
| Themes         | Light + dark via `light-dark()` on `color-scheme: light dark`, following the system; no in-app toggle |
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
2. Whether the tree is virtualised from the start. The standard nodeset is
   ~35 000 nodes, so probably yes, which constrains how it can be styled.
3. Icon set for the eight node classes — Material Symbols has no OPC UA
   vocabulary, so these are analogies that need choosing deliberately.
4. Density. M3's defaults are touch-sized; a nodeset tree wants to be tighter.
   Likely a compact density override rather than fighting each component.
