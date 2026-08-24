//! The two channels the editor's panes need across panes: go somewhere, and open a dialog. Both are
//! provided by the editor route, so any pane can ask without owning the tree or the dialog stack.

use dioxus::prelude::*;
use uanedit::edit::ReferenceKey;
use uanedit::rules::finding::Finding;
use uanedit::types::node_id::NodeId;

use crate::components::Icon;
use crate::views::editor::diagnostics::OverrideResult;
use crate::views::editor::instantiate::{
    InstantiateDialog,
    Seed,
};
use crate::views::editor::lifecycle::{
    CreateDialog,
    DeleteDialog,
};
use crate::views::editor::references::{
    AddReferenceDialog,
    RetargetDialog,
};
use crate::views::editor::subtype::SubtypeDialog;

/// Take the user somewhere: a node in the tree, or the validation tab.
#[derive(Clone, Copy)]
pub struct Navigate {
    pub target: Signal<Option<NodeId>>,
    pub show_validation: Signal<bool>,
}

impl Default for Navigate {
    fn default() -> Self {
        Self::new()
    }
}

impl Navigate {
    pub fn new() -> Self {
        Self {
            target: Signal::new(None),
            show_validation: Signal::new(false),
        }
    }

    pub fn to(
        mut self,
        node: NodeId,
    ) {
        self.target.set(Some(node));
    }

    pub fn to_validation(mut self) {
        self.show_validation.set(true);
    }
}

/// The one dialog the editor has open, if any, and what an override last let through.
#[derive(Clone, Copy)]
pub struct Dialogs {
    pub request: Signal<Option<Request>>,
    pub overridden: Signal<Vec<Finding>>,
}

#[derive(Clone, PartialEq)]
pub enum Request {
    /// Create a node, with the parent or supertype the tree had selected.
    Create {
        anchor: Option<NodeId>,
    },
    /// Create an instance of a type with everything its modelling rules ask for.
    Instantiate {
        seed: Seed,
    },
    /// Create a subtype of the selected type, with the declarations it overrides.
    Subtype {
        supertype: Option<NodeId>,
    },
    Delete {
        node: NodeId,
    },
    AddReference {
        node: NodeId,
    },
    /// Point an existing reference at another node.
    Retarget {
        reference: ReferenceKey,
        held_by: NodeId,
    },
}

impl Default for Dialogs {
    fn default() -> Self {
        Self::new()
    }
}

impl Dialogs {
    pub fn new() -> Self {
        Self {
            request: Signal::new(None),
            overridden: Signal::new(Vec::new()),
        }
    }

    pub fn open(
        mut self,
        request: Request,
    ) {
        self.request.set(Some(request));
    }

    pub fn close(mut self) {
        self.request.set(None);
    }

    /// Puts what an override introduced in front of the user, in place of whatever asked for it.
    pub fn report(
        mut self,
        findings: Vec<Finding>,
    ) {
        self.request.set(None);
        self.overridden.set(findings);
    }
}

#[component]
pub fn DialogHost() -> Element {
    let dialogs: Dialogs = use_context();
    let overridden = dialogs.overridden.read().clone();
    if !overridden.is_empty() {
        return rsx! {
            OverrideReport { findings: overridden }
        };
    }

    match dialogs.request.read().clone() {
        None => rsx! {},
        Some(Request::Create { anchor }) => rsx! {
            CreateDialog { anchor }
        },
        Some(Request::Instantiate { seed }) => rsx! {
            InstantiateDialog { seed }
        },
        Some(Request::Subtype { supertype }) => rsx! {
            SubtypeDialog { supertype }
        },
        Some(Request::Delete { node }) => rsx! {
            DeleteDialog { node }
        },
        Some(Request::AddReference { node }) => rsx! {
            AddReferenceDialog { node }
        },
        Some(Request::Retarget { reference, held_by }) => rsx! {
            RetargetDialog { reference, held_by }
        },
    }
}

/// The moment after an override: what it introduced, and where to go and look at it.
#[component]
fn OverrideReport(findings: Vec<Finding>) -> Element {
    let mut dialogs: Dialogs = use_context();
    let navigate: Navigate = use_context();

    rsx! {
        Dialog {
            title: "Applied past the engine".to_owned(),
            icon: "bolt",
            subtitle: "The operation was performed and the findings it introduced were kept.".to_owned(),
            onclose: move |_| dialogs.overridden.set(Vec::new()),
            div { class: "dialog__body",
                OverrideResult { findings }
            }
            footer { class: "dialog__actions",
                button {
                    class: "button text",
                    onclick: move |_| dialogs.overridden.set(Vec::new()),
                    "Close"
                }
                button {
                    class: "button tonal",
                    onclick: move |_| {
                        navigate.to_validation();
                        dialogs.overridden.set(Vec::new());
                    },
                    Icon { name: "rule", class: "small" }
                    "Open the Validation tab"
                }
            }
        }
    }
}

/// The M3 dialog shell: scrim, surface, a head that names what is being done, and a body.
#[component]
pub fn Dialog(
    title: String,
    icon: &'static str,
    #[props(default)] subtitle: String,
    #[props(default)] wide: bool,
    onclose: EventHandler<()>,
    children: Element,
) -> Element {
    let class = match wide {
        true => "dialog wide",
        false => "dialog",
    };

    rsx! {
        div { class: "scrim", onclick: move |_| onclose.call(()),
            div {
                class: class,
                onclick: move |event| event.stop_propagation(),
                header { class: "dialog__head",
                    Icon { name: icon, class: "small" }
                    div { class: "dialog__titles",
                        span { class: "dialog__title type-title-small", {title} }
                        if !subtitle.is_empty() {
                            span { class: "dialog__subtitle type-label-small", {subtitle} }
                        }
                    }
                    div { class: "dialog__head-spacer" }
                    button {
                        class: "icon-button tiny",
                        title: "Close",
                        onclick: move |_| onclose.call(()),
                        Icon { name: "close", class: "small" }
                    }
                }
                {children}
            }
        }
    }
}
