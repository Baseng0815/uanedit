//! The Material 3 dialog the document-level surfaces share, and the stylesheet they all need.

use dioxus::prelude::*;

use crate::components::Icon;

const DOC_CSS: Asset = asset!("/assets/document.css");

/// Links the document surface's stylesheet. Head links are deduplicated by href, so every surface
/// that needs it may ask for it.
#[component]
pub fn DocumentStyles() -> Element {
    rsx! {
        document::Stylesheet { href: DOC_CSS }
    }
}

#[component]
pub fn Dialog(
    title: String,
    icon: &'static str,
    #[props(default)] wide: bool,
    subtitle: Option<String>,
    onclose: EventHandler<()>,
    actions: Option<Element>,
    children: Element,
) -> Element {
    rsx! {
        DocumentStyles {}
        div { class: "dialog-scrim", onclick: move |_| onclose.call(()),
            div {
                class: if wide { "dialog wide" } else { "dialog" },
                onclick: move |event| event.stop_propagation(),
                header { class: "dialog__head",
                    Icon { name: icon }
                    div { class: "dialog__titles",
                        span { class: "type-title", {title} }
                        if let Some(subtitle) = subtitle {
                            span { class: "dialog__subtitle type-body-small mono", {subtitle} }
                        }
                    }
                    div { class: "dialog__spacer" }
                    button {
                        class: "icon-button",
                        title: "Close",
                        onclick: move |_| onclose.call(()),
                        Icon { name: "close" }
                    }
                }
                div { class: "dialog__body", {children} }
                if let Some(actions) = actions {
                    div { class: "dialog__actions", {actions} }
                }
            }
        }
    }
}
