use dioxus::prelude::*;

/// A Material Symbols Rounded ligature; `name` is the icon's codepoint name, e.g. `folder_open`.
#[component]
pub fn Icon(
    name: &'static str,
    #[props(default)] class: &'static str,
) -> Element {
    rsx! {
        span { class: "material-symbols-rounded {class}", aria_hidden: true, {name} }
    }
}
