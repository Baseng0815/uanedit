use dioxus::prelude::*;

use crate::components::Icon;

/// One column of the editor's three-pane workspace, empty until its wave lands.
#[component]
pub fn Pane(
    title: &'static str,
    icon: &'static str,
    hint: String,
) -> Element {
    rsx! {
        section { class: "pane",
            header { class: "pane__header",
                Icon { name: icon, class: "small" }
                span { class: "pane__title", {title} }
            }
            div { class: "pane__body",
                div { class: "pane__placeholder",
                    Icon { name: icon }
                    span { class: "type-body-small", {hint} }
                }
            }
        }
    }
}
