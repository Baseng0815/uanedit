//! The export preview: one generated file, syntax-highlighted, before it is saved anywhere.

use dioxus::prelude::*;
use dioxus_code::{
    Code,
    CodeTheme,
    Language,
    SourceCode,
    Theme as SyntaxTheme,
};

use crate::components::{
    Dialog,
    Icon,
    Theme,
    use_theme,
};

/// What the preview renders before it stops. `Code` emits one element per token, so a 4 MB
/// nodeset would be hundreds of thousands of DOM nodes; the raw link carries the rest.
const MAX_LINES: usize = 800;
const MAX_BYTES: usize = 128 * 1024;

pub const DEFAULT_ZOOM: u8 = 12;
const ZOOM_RANGE: std::ops::RangeInclusive<u8> = 9..=22;

#[component]
pub fn FilePreview(
    name: String,
    text: String,
    /// The blob URL the export prepared, for reading the whole file outside the dialog.
    url: Option<String>,
    /// Type size in pixels, owned by the caller so it survives switching between files.
    zoom: Signal<u8>,
    onclose: EventHandler<()>,
) -> Element {
    let theme = use_theme();
    let (shown, total, truncated) = clamp(&text);
    let lines = shown.lines().count();
    let size = zoom();

    rsx! {
        Dialog {
            title: name.clone(),
            icon: "code",
            full: true,
            subtitle: measure(lines, total, truncated),
            onclose,
            actions: rsx! {
                Zoom { zoom }
                if let Some(url) = url {
                    a {
                        class: "button text",
                        href: "{url}",
                        target: "_blank",
                        rel: "noopener",
                        Icon { name: "open_in_new", class: "small" }
                        "Open raw"
                    }
                }
                button { class: "button", onclick: move |_| onclose.call(()), "Close" }
            },
            div { class: "preview", style: "--preview-type-size: {size}px",
                pre { class: "preview__gutter", aria_hidden: true, {gutter(lines)} }
                Code {
                    src: SourceCode::new(language(&name), shown),
                    theme: syntax_theme(theme()),
                }
            }
        }
    }
}

#[component]
fn Zoom(mut zoom: Signal<u8>) -> Element {
    let size = zoom();

    rsx! {
        div { class: "preview__zoom",
            button {
                class: "icon-button tiny",
                disabled: size == *ZOOM_RANGE.start(),
                title: "Smaller type",
                onclick: move |_| zoom.set(size.saturating_sub(1).max(*ZOOM_RANGE.start())),
                Icon { name: "remove", class: "small" }
            }
            button {
                class: "preview__zoom-size type-label-small mono",
                disabled: size == DEFAULT_ZOOM,
                title: "Back to the default size",
                onclick: move |_| zoom.set(DEFAULT_ZOOM),
                "{size}px"
            }
            button {
                class: "icon-button tiny",
                disabled: size == *ZOOM_RANGE.end(),
                title: "Larger type",
                onclick: move |_| zoom.set(size.saturating_add(1).min(*ZOOM_RANGE.end())),
                Icon { name: "add", class: "small" }
            }
        }
    }
}

/// The line numbers as one `pre`, which is what keeps them aligned: the column beside the code
/// shares its type size and line height, so every line box is the same height in both.
fn gutter(lines: usize) -> String {
    (1..=lines).fold(String::new(), |mut out, line| {
        if line > 1 {
            out.push('\n');
        }
        out.push_str(&line.to_string());
        out
    })
}

/// The grammar a generated file gets, by the extension the export gave it.
fn language(name: &str) -> Language {
    let lower = name.to_lowercase();
    if lower.ends_with(".rs") {
        Language::Rust
    } else if lower.ends_with(".c") || lower.ends_with(".h") {
        Language::C
    } else {
        Language::Xml
    }
}

/// Follows the app's own override rather than `prefers-color-scheme`, which would ignore a light
/// theme forced on a dark system.
fn syntax_theme(theme: Theme) -> CodeTheme {
    match theme {
        Theme::System => CodeTheme::system(SyntaxTheme::GITHUB_LIGHT, SyntaxTheme::GITHUB_DARK),
        Theme::Light => CodeTheme::fixed(SyntaxTheme::GITHUB_LIGHT),
        Theme::Dark => CodeTheme::fixed(SyntaxTheme::GITHUB_DARK),
    }
}

/// Cuts at whichever limit comes first, always on a line boundary.
fn clamp(text: &str) -> (String, usize, bool) {
    let total = text.lines().count();
    let mut shown = String::new();
    for line in text.lines().take(MAX_LINES) {
        if shown.len() + line.len() > MAX_BYTES {
            break;
        }
        shown.push_str(line);
        shown.push('\n');
    }
    let truncated = shown.lines().count() < total;
    (shown, total, truncated)
}

fn measure(
    lines: usize,
    total: usize,
    truncated: bool,
) -> String {
    match truncated {
        true => format!("first {lines} of {total} lines — open raw for all of it"),
        false => format!("{total} lines"),
    }
}
