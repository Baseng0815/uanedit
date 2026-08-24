//! The invariant the codec exists for: a file read and written without edits comes back out byte
//! for byte, an edited one differs only where the edit was, and what this crate writes from
//! nothing validates against the schema.

#![cfg(feature = "xml")]

use std::path::{
    Path,
    PathBuf,
};
use std::process::Command;
use std::time::Instant;

use uanedit::error::DocumentError;
use uanedit::ids;
use uanedit::nodes::{
    Node,
    NodeHeader,
    Object,
    ObjectType,
    Reference,
    Variable,
};
use uanedit::nodeset::{
    ModelTableEntry,
    NamespaceTable,
    NodeSet,
};
use uanedit::types::{
    LocalizedText,
    NodeId,
    NodeIdRef,
    QualifiedName,
    Variant,
};
use uanedit::xml::{
    self,
    Document,
};

mod awkward;

fn fixture(name: &str) -> Option<(PathBuf, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let Ok(source) = std::fs::read_to_string(&path) else {
        eprintln!(
            "SKIPPED: {} is not in the working tree; the fixtures are gitignored, fetch it to run this test",
            path.display()
        );
        return None;
    };
    Some((path, source))
}

/// Reports the first byte that differs, since a four-megabyte diff is unreadable.
fn assert_identical(
    label: &str,
    source: &str,
    written: &str,
) {
    if source == written {
        return;
    }
    let at = source
        .bytes()
        .zip(written.bytes())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| source.len().min(written.len()));
    let window = |text: &str| {
        let from = at.saturating_sub(120);
        text.get(from..(at + 120).min(text.len()))
            .unwrap_or_default()
            .to_owned()
    };
    panic!(
        "{label}: output differs at byte {at} (read {} bytes, wrote {} bytes)\n  read:    {:?}\n  written: {:?}",
        source.len(),
        written.len(),
        window(source),
        window(written),
    );
}

#[test]
fn standard_nodeset_round_trips_byte_for_byte() {
    let Some((_, source)) = fixture("Opc.Ua.NodeSet2.xml") else {
        return;
    };
    let reading = Instant::now();
    let document = Document::parse(&source).expect("the standard nodeset parses");
    let read = reading.elapsed();

    let writing = Instant::now();
    let written = document.write();
    let wrote = writing.elapsed();

    eprintln!(
        "Opc.Ua.NodeSet2.xml: {} bytes, {} nodes, read in {read:?}, written in {wrote:?}",
        source.len(),
        document.nodeset().len(),
    );
    assert_identical("Opc.Ua.NodeSet2.xml", &source, &written);
}

#[test]
fn companion_nodeset_round_trips_byte_for_byte() {
    let Some((_, source)) = fixture("Opc.Ua.Di.NodeSet2.xml") else {
        return;
    };
    let document = Document::parse(&source).expect("the DI nodeset parses");
    assert_identical("Opc.Ua.Di.NodeSet2.xml", &source, &document.write());
}

/// Writing from the model alone has to reproduce that model, which is what catches a reader that
/// quietly drops something the splicing writer would never have been asked for.
#[test]
fn re_reading_what_the_writer_produces_gives_the_same_model() {
    for name in ["Opc.Ua.NodeSet2.xml", "Opc.Ua.Di.NodeSet2.xml"] {
        let Some((_, source)) = fixture(name) else {
            continue;
        };
        let document = Document::parse(&source).expect("the nodeset parses");
        let written = xml::write(document.nodeset());
        let again = Document::parse(&written).expect("what the writer produced parses");
        assert_eq!(again.nodeset(), document.nodeset(), "{name} lost something in writing");
    }
}

/// The splice writer keeps a node's source bytes only while the model still matches them; the
/// moment an edit lands anywhere in that node, the fragments below are what the file gets. They
/// therefore have to reproduce the node the document already holds, byte for byte.
#[test]
fn re_rendering_a_node_reproduces_its_source() {
    let mut documents = vec![
        ("awkward", awkward::AWKWARD.to_owned()),
        ("bom-crlf", awkward::BOM_CRLF.to_owned()),
    ];
    for name in ["Opc.Ua.NodeSet2.xml", "Opc.Ua.Di.NodeSet2.xml"] {
        if let Some((_, source)) = fixture(name) {
            documents.push((name, source));
        }
    }
    for (name, source) in documents {
        let document = Document::parse(&source).expect("the nodeset parses");
        let mut failures = Vec::new();
        for node in document.nodeset().iter() {
            let region = document
                .layout()
                .nodes
                .get(node.node_id())
                .expect("every node that was read has a region");
            let rendered = document.render_node(node);
            for (part, written, span) in [
                ("start tag", &rendered.tag, region.tag),
                ("body", &rendered.body, region.body),
                ("end tag", &rendered.end, region.end),
            ] {
                let read = span.of(&source);
                if written != read {
                    failures.push(format!("{} {part}\n  read:    {read:?}\n  written: {written:?}", node.node_id()));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{name}: {} of {} nodes re-render differently:\n{}",
            failures.len(),
            document.nodeset().len(),
            failures
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
}

#[test]
fn awkward_nodeset_round_trips_byte_for_byte() {
    let document = Document::parse(awkward::AWKWARD).expect("the awkward nodeset parses");
    assert_identical("awkward", awkward::AWKWARD, &document.write());
}

#[test]
fn a_byte_order_mark_and_crlf_survive() {
    let document = Document::parse(awkward::BOM_CRLF).expect("the BOM and CRLF nodeset parses");
    assert_identical("bom-crlf", awkward::BOM_CRLF, &document.write());
}

/// The awkward cases only count as preserved if they were understood, so check what reading made.
#[test]
fn awkward_nodeset_reads_what_it_says() {
    let document = Document::parse(awkward::AWKWARD).expect("the awkward nodeset parses");
    let nodeset = document.nodeset();

    let guid_node = nodeset
        .iter()
        .find(|node| node.browse_name().name == "GuidNode")
        .expect("the Guid node is there");
    assert_eq!(guid_node.header().display_name.len(), 2);
    assert_eq!(guid_node.header().unknown_attributes.len(), 1);
    assert_eq!(guid_node.header().unknown_elements.len(), 1);

    let Some(Node::Variable(variable)) = nodeset
        .iter()
        .find(|node| node.browse_name().name == "OpaqueNode")
    else {
        panic!("the opaque node is there and is a Variable");
    };
    let Some(Variant::Array(values)) = &variable.value else {
        panic!("its value is an array, not {:?}", variable.value);
    };
    assert_eq!(values.len(), 1);
    assert_eq!(nodeset.aliases.len(), 3);
    assert_eq!(nodeset.extensions.len(), 1);
    assert!(document.report().is_clean(), "{:?}", document.report().findings);
    assert_eq!(document.report().preserved.len(), 3);
}

/// A `UANodeSetChanges` file is out of scope by design, and says so rather than failing obscurely.
#[test]
fn a_changes_file_is_refused_politely() {
    let error = Document::parse(awkward::CHANGES).expect_err("a changes file is refused");
    assert_eq!(error, DocumentError::NodeSetChanges);
    assert!(error.to_string().contains("apply the changes"), "{error}");
}

/// Editing one attribute may rewrite that node's start tag and nothing else in the document.
#[test]
fn an_edit_changes_only_the_node_it_touches() {
    let Some((_, source)) = fixture("Opc.Ua.Di.NodeSet2.xml") else {
        return;
    };
    let mut document = Document::parse(&source).expect("the DI nodeset parses");
    let target = document
        .nodeset()
        .iter()
        .find(|node| matches!(node, Node::Variable(_)) && node.header().symbolic_name.is_some())
        .expect("the DI nodeset has a Variable with a SymbolicName")
        .node_id()
        .clone();
    document
        .nodeset_mut()
        .node_mut(&target)
        .expect("the node is still there")
        .header_mut()
        .symbolic_name = Some("RenamedByTheTest".to_owned());

    let written = document.write();
    let changed: Vec<_> = source
        .lines()
        .zip(written.lines())
        .filter(|(before, after)| before != after)
        .collect();
    assert_eq!(source.lines().count(), written.lines().count(), "the line count moved");
    assert_eq!(changed.len(), 1, "more than the edited start tag changed: {changed:#?}");
    assert!(changed[0].1.contains("RenamedByTheTest"), "{:?}", changed[0]);

    let again = Document::parse(&written).expect("the edited nodeset parses");
    assert_eq!(
        again
            .nodeset()
            .node(&target)
            .and_then(|node| node.header().symbolic_name.as_deref()),
        Some("RenamedByTheTest"),
    );
}

/// An edit rewrites the one thing it changed. Everything else in that start tag keeps the document's
/// own order and spelling, the empty children stay empty rather than disappearing, and a deletion
/// does not take the comment in front of the node with it.
#[test]
fn an_edit_leaves_the_documents_own_spellings_alone() {
    let document = Document::parse(awkward::LEXICAL).expect("the lexical nodeset parses");
    assert_identical("lexical", awkward::LEXICAL, &document.write());

    let odd = NodeId::numeric(1, 1);
    let mut edited = document.nodeset().clone();
    edited
        .node_mut(&odd)
        .expect("the odd node is there")
        .header_mut()
        .symbolic_name = Some("Renamed".to_owned());
    let written = document.write_nodeset(&edited);
    let tag = written
        .lines()
        .find(|line| line.contains("1:Odd"))
        .expect("the edited start tag");
    assert!(tag.contains(r#"MinimumSamplingInterval="1.0E3""#), "{tag}");
    assert!(tag.contains(r#"AccessLevel="03""#), "{tag}");
    assert!(tag.contains(r#"ArrayDimensions=" 2,3 ""#), "{tag}");
    assert!(tag.contains(r#"SymbolicName="Renamed""#), "{tag}");
    assert!(tag.find("BrowseName") < tag.find("NodeId"), "the source order moved: {tag}");
    for kept in ["<References />", "<RolePermissions />", "<Extensions />"] {
        assert!(written.contains(kept), "{kept} went missing:\n{written}");
    }
    assert!(written.contains("</DisplayName>\n    <vendor:Note"), "{written}");

    let mut pruned = document.nodeset().clone();
    pruned.nodes.shift_remove(&NodeId::numeric(1, 2));
    let written = document.write_nodeset(&pruned);
    assert!(!written.contains("1:Doomed"), "{written}");
    assert!(
        written.contains("<!-- a comment in front of the node that gets deleted -->\n  <UAObject NodeId=\"ns=1;i=3\""),
        "{written}"
    );
}

/// A nodeset built in code, written out, and handed to a validator this crate did not write.
#[test]
fn a_nodeset_written_from_nothing_validates_against_the_schema() {
    let nodeset = scratch_nodeset();
    let written = xml::write(&nodeset);
    validate(&written, "uanedit-scratch.NodeSet2.xml");

    let read = Document::parse(&written).expect("what the writer produced parses");
    assert_eq!(read.nodeset(), &nodeset);
}

fn scratch_nodeset() -> NodeSet {
    let mut nodeset = NodeSet {
        namespaces: NamespaceTable::new(vec!["http://example.org/scratch/".to_owned()]),
        models: vec![ModelTableEntry {
            version: Some("1.0.0".to_owned()),
            model_version: Some("1.0.0".to_owned()),
            publication_date: Some("2026-01-01T00:00:00Z".parse().expect("a valid dateTime")),
            ..ModelTableEntry::new("http://example.org/scratch/")
        }],
        ..NodeSet::default()
    };
    nodeset
        .aliases
        .insert("HasTypeDefinition", ids::HAS_TYPE_DEFINITION);
    nodeset.aliases.insert("HasSubtype", ids::HAS_SUBTYPE);

    let mut scratch_type = ObjectType {
        header: NodeHeader::new(NodeId::numeric(1, 1000), QualifiedName::new(1, "ScratchType")),
        is_abstract: false,
    };
    scratch_type.header.display_name = vec![
        LocalizedText::new("Scratch type"),
        LocalizedText::localized("de", "Kratztyp"),
    ];
    scratch_type.header.references = vec![Reference::inverse(
        NodeIdRef::alias("HasSubtype"),
        ids::BASE_OBJECT_TYPE,
    )];
    nodeset.insert(scratch_type);

    let mut instance = Object {
        header: NodeHeader::new(NodeId::numeric(1, 1001), QualifiedName::new(1, "Scratch")),
        ..Object::default()
    };
    instance.header.display_name = vec![LocalizedText::new("Scratch")];
    instance.header.references = vec![Reference::forward(
        NodeIdRef::alias("HasTypeDefinition"),
        NodeId::numeric(1, 1000),
    )];
    instance.instance.parent_node_id = Some(NodeId::numeric(0, 85).into());
    nodeset.insert(instance);

    let mut counter = Variable {
        header: NodeHeader::new(NodeId::numeric(1, 1002), QualifiedName::new(1, "Counter")),
        data_type: ids::UINTEGER.into(),
        value: Some(Variant::UInt32(7)),
        ..Variable::default()
    };
    counter.header.display_name = vec![LocalizedText::new("Counter")];
    counter.header.description = vec![LocalizedText::localized("en", "A value & a <bracket>")];
    counter.header.references = vec![Reference::forward(
        NodeIdRef::alias("HasTypeDefinition"),
        ids::BASE_DATA_VARIABLE_TYPE,
    )];
    nodeset.insert(counter);
    nodeset
}

/// Checks a document against the UANodeSet schema with a validator this crate did not write.
fn validate(
    document: &str,
    name: &str,
) {
    let schema = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root")
        .join("UANodeSet.xsd");
    if !schema.exists() {
        eprintln!("SKIPPED: {} is not in the working tree; fetch the schema to run this test", schema.display());
        return;
    }
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, document).expect("the temporary file is writable");

    let output = match Command::new("xmllint")
        .arg("--noout")
        .arg("--schema")
        .arg(&schema)
        .arg(&path)
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("SKIPPED: xmllint is not on PATH ({error})");
            return;
        }
    };
    assert!(
        output.status.success(),
        "xmllint rejected {}:\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stderr),
    );
}
