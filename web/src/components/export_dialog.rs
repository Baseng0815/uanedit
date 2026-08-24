//! The export dialog: what leaves the editor, and whether it leaves as one file, several, or a
//! link to open in a tab.

use dioxus::prelude::*;

use crate::components::{
    DEFAULT_ZOOM,
    Dialog,
    FilePreview,
    Icon,
};
use crate::export::{
    self,
    ExportRequest,
    archive_name,
};
use crate::session::{
    EditorHandle,
    Status,
};

#[component]
pub fn ExportDialog(
    file: String,
    onclose: EventHandler<()>,
) -> Element {
    let handle: EditorHandle = use_context();
    let mut request = use_signal(ExportRequest::default);
    // What View generated, empty until it has run.
    let mut generated = use_signal(Vec::<Prepared>::new);
    let mut preparing = use_signal(|| false);
    let mut showing = use_signal(|| None::<Prepared>);
    // Held here rather than in the preview, so it survives switching between files.
    let zoom = use_signal(|| DEFAULT_ZOOM);

    let asked = request();
    let names = asked.file_names(&file);
    let prepared = generated();
    let working = preparing();

    let view = move |_| {
        if *preparing.peek() {
            return;
        }
        preparing.set(true);
        spawn(async move {
            let prepared = match handle.collect(request()).await {
                Ok(bundle) => export::link(&bundle.files)
                    .await
                    .map(|urls| Prepared::zip(bundle.files, urls)),
                Err(error) => Err(error),
            };
            preparing.set(false);
            match prepared {
                Ok(prepared) => {
                    export::revoke(
                        generated
                            .peek()
                            .iter()
                            .map(|file| file.url.clone())
                            .collect(),
                    );
                    generated.set(prepared);
                }
                Err(error) => handle.say(Status::error(format!("Preparing files failed: {error}"))),
            }
        });
    };

    let export = move |_| {
        handle.export(request());
        onclose.call(());
    };

    rsx! {
        Dialog {
            title: "Export",
            icon: "download",
            subtitle: file.clone(),
            onclose,
            actions: rsx! {
                button { class: "button text", onclick: move |_| onclose.call(()), "Cancel" }
                button {
                    class: "button tonal",
                    disabled: asked.is_empty() || working,
                    title: "Generate the files and read them here, instead of saving them",
                    onclick: view,
                    Icon { name: "visibility", class: "small" }
                    if working {
                        "Preparing…"
                    } else {
                        "View"
                    }
                }
                button {
                    class: "button",
                    disabled: asked.is_empty() || working,
                    onclick: export,
                    Icon { name: "download", class: "small" }
                    "Download"
                }
            },
            div { class: "doc-form",
                div { class: "field",
                    span { class: "field__label", "Contents" }
                    label { class: "field__toggle",
                        input {
                            r#type: "checkbox",
                            checked: asked.xml,
                            onchange: move |event| request.write().xml = event.checked(),
                        }
                        span { class: "field__toggle-label", "NodeSet2 XML" }
                    }
                    span { class: "field__hint indented",
                        "The XML a save would write, unsaved edits included. Nothing is written to disk."
                    }
                    label { class: "field__toggle",
                        input {
                            r#type: "checkbox",
                            checked: asked.c_sources,
                            onchange: move |event| request.write().c_sources = event.checked(),
                        }
                        span { class: "field__toggle-label", "open62541 C sources" }
                    }
                    span { class: "field__hint indented",
                        "The .c/.h pair the open62541 nodeset compiler would produce, with every dependency treated as existing."
                    }
                    label { class: if asked.c_sources { "field__toggle indented" } else { "field__toggle indented muted" },
                        input {
                            r#type: "checkbox",
                            checked: asked.rust_bindings,
                            disabled: !asked.c_sources,
                            onchange: move |event| request.write().rust_bindings = event.checked(),
                        }
                        span { class: "field__toggle-label", "Rust bindings" }
                    }
                }
                div { class: "field",
                    span { class: "field__label", "Delivery" }
                    label { class: "field__toggle",
                        input {
                            r#type: "radio",
                            name: "export-delivery",
                            checked: !asked.archive,
                            onchange: move |_| request.write().archive = false,
                        }
                        span { class: "field__toggle-label", "Separate files" }
                    }
                    label { class: "field__toggle",
                        input {
                            r#type: "radio",
                            name: "export-delivery",
                            checked: asked.archive,
                            onchange: move |_| request.write().archive = true,
                        }
                        span { class: "field__toggle-label", "One ZIP archive" }
                    }
                    span { class: "field__hint",
                        "Delivery applies to Download. View always prepares the files one by one."
                    }
                }
                div { class: "field",
                    span { class: "field__label", "Files" }
                    if names.is_empty() {
                        span { class: "field__static", "Nothing selected." }
                    } else if asked.archive {
                        ul { class: "export__files mono",
                            li { {archive_name(&file)} }
                            ul { class: "export__files nested",
                                for name in names.iter() {
                                    FileEntry {
                                        key: "{name}",
                                        name: name.clone(),
                                        prepared: found(&prepared, name),
                                        onpreview: move |file| showing.set(Some(file)),
                                    }
                                }
                            }
                        }
                    } else {
                        ul { class: "export__files mono",
                            for name in names.iter() {
                                FileEntry {
                                    key: "{name}",
                                    name: name.clone(),
                                    prepared: found(&prepared, name),
                                    onpreview: move |file| showing.set(Some(file)),
                                }
                            }
                        }
                    }
                    if !prepared.is_empty() {
                        span { class: "field__hint",
                            "Click a file to read it highlighted, or the arrow for the raw text in a new tab."
                        }
                    }
                }
            }
        }
        if let Some(file) = showing() {
            FilePreview {
                name: file.name,
                text: file.text,
                url: file.url,
                zoom,
                onclose: move |_: ()| showing.set(None),
            }
        }
    }
}

/// One file View generated: its text, for the preview, and its blob URL, for a raw tab.
#[derive(Clone, PartialEq)]
struct Prepared {
    name: String,
    text: String,
    url: String,
}

impl Prepared {
    fn zip(
        files: Vec<(String, String)>,
        urls: Vec<String>,
    ) -> Vec<Self> {
        files
            .into_iter()
            .zip(urls)
            .map(|((name, text), url)| Self { name, text, url })
            .collect()
    }
}

/// One file: readable two ways once View has prepared it, its bare name until then.
#[component]
fn FileEntry(
    name: String,
    prepared: Option<Prepared>,
    onpreview: EventHandler<Prepared>,
) -> Element {
    rsx! {
        li {
            if let Some(file) = prepared {
                button {
                    class: "export__link",
                    title: "Read it here, highlighted",
                    onclick: {
                        let file = file.clone();
                        move |_| onpreview.call(file.clone())
                    },
                    {name.clone()}
                }
                a {
                    class: "export__raw",
                    href: file.url,
                    target: "_blank",
                    rel: "noopener",
                    title: "Open the raw text in a new tab",
                    Icon { name: "open_in_new", class: "small" }
                }
            } else {
                {name.clone()}
            }
        }
    }
}

fn found(
    prepared: &[Prepared],
    name: &str,
) -> Option<Prepared> {
    prepared.iter().find(|file| file.name == name).cloned()
}
