//! The right pane: references and validation, one tab each.

use dioxus::prelude::*;

use crate::components::Icon;
use crate::session::EditorHandle;
use crate::views::editor::counted;
use crate::views::editor::references::ReferencesPanel;
use crate::views::editor::shell::Navigate;
use crate::views::editor::validation::{
    ValidationPanel,
    counts,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    References,
    Validation,
}

#[component]
pub fn RightPane() -> Element {
    let handle: EditorHandle = use_context();
    let navigate: Navigate = use_context();
    let mut tab = use_signal(|| Tab::References);

    let totals = use_memo(move || {
        let _ = *handle.revision.read();
        counts(handle)
    });

    // An override points at the findings it introduced, which are in the other tab.
    use_effect(move || {
        let mut asked = navigate.show_validation;
        if !*asked.read() {
            return;
        }
        tab.set(Tab::Validation);
        asked.set(false);
    });

    let now = *tab.read();
    let totals = *totals.read();
    let loud = totals.errors > 0;

    rsx! {
        section { class: "pane",
            header { class: "pane__header tabs",
                button {
                    class: if now == Tab::References { "tab on" } else { "tab" },
                    onclick: move |_| tab.set(Tab::References),
                    Icon { name: "link", class: "small" }
                    "References"
                }
                button {
                    class: if now == Tab::Validation { "tab on" } else { "tab" },
                    onclick: move |_| tab.set(Tab::Validation),
                    Icon { name: "rule", class: "small" }
                    "Validation"
                    span {
                        class: if loud { "tab__badge error" } else { "tab__badge" },
                        title: "{counted(totals.errors, \"error\")}, {counted(totals.warnings, \"warning\")}",
                        "{totals.errors + totals.warnings}"
                    }
                }
            }
            div { class: "pane__body",
                match now {
                    Tab::References => rsx! {
                        ReferencesPanel {}
                    },
                    Tab::Validation => rsx! {
                        ValidationPanel {}
                    },
                }
            }
        }
    }
}
