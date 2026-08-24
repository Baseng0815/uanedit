//! The new-nodeset dialog: the three things a blank document needs before it exists.

use dioxus::prelude::*;

use crate::api::create_file;
use crate::components::{
    Dialog,
    Icon,
};
use crate::route::Route;

const DEFAULT_VERSION: &str = "1.0.0";

#[component]
pub fn CreateFileDialog(onclose: EventHandler<()>) -> Element {
    let mut name = use_signal(String::new);
    let mut model_uri = use_signal(String::new);
    let mut version = use_signal(|| DEFAULT_VERSION.to_owned());
    let mut failure = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let navigator = use_navigator();

    let ready = !name.read().trim().is_empty() && !model_uri.read().trim().is_empty();
    let working = busy();

    let create = move |_| {
        let file = name.peek().trim().to_owned();
        let uri = model_uri.peek().trim().to_owned();
        let pinned = match version.peek().trim() {
            "" => DEFAULT_VERSION.to_owned(),
            given => given.to_owned(),
        };
        if file.is_empty() || uri.is_empty() || *busy.peek() {
            return;
        }
        busy.set(true);
        failure.set(None);
        spawn(async move {
            let (mut busy, mut failure) = (busy, failure);
            match create_file(file, uri, pinned).await {
                Ok(created) => {
                    onclose.call(());
                    navigator.push(Route::Editor { file: created.name });
                }
                Err(error) => {
                    busy.set(false);
                    failure.set(Some(error.to_string()));
                }
            }
        });
    };

    rsx! {
        Dialog {
            title: "New nodeset",
            icon: "note_add",
            onclose,
            actions: rsx! {
                button { class: "button text", onclick: move |_| onclose.call(()), "Cancel" }
                button {
                    class: "button",
                    disabled: !ready || working,
                    onclick: create,
                    Icon { name: "add", class: "small" }
                    if working {
                        "Creating…"
                    } else {
                        "Create"
                    }
                }
            },
            div { class: "doc-form",
                if let Some(message) = failure() {
                    div { class: "notice",
                        Icon { name: "error" }
                        span { {message} }
                    }
                }
                div { class: "field",
                    span { class: "field__label", "File name" }
                    input {
                        class: "field__input mono",
                        value: "{name}",
                        placeholder: "MyModel",
                        autofocus: true,
                        oninput: move |event| name.set(event.value()),
                    }
                    span { class: "field__hint",
                        ".NodeSet2.xml is appended unless the name already ends in .xml"
                    }
                }
                div { class: "field",
                    span { class: "field__label", "ModelUri" }
                    input {
                        class: "field__input mono",
                        value: "{model_uri}",
                        placeholder: "http://example.org/MyModel/",
                        oninput: move |event| model_uri.set(event.value()),
                    }
                    span { class: "field__hint",
                        "The namespace this file defines. It becomes namespace index 1 and the first model entry."
                    }
                }
                div { class: "field",
                    span { class: "field__label", "Version" }
                    input {
                        class: "field__input mono",
                        value: "{version}",
                        placeholder: DEFAULT_VERSION,
                        oninput: move |event| version.set(event.value()),
                    }
                    span { class: "field__hint",
                        "Written as both Version and ModelVersion; PublicationDate is stamped now."
                    }
                }
            }
        }
    }
}
