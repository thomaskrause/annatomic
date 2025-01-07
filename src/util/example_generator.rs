use graphannis_core::graph::{
    update::{GraphUpdate, UpdateEvent},
    ANNIS_NS,
};

/// Create update events for the following corpus structure:
///
/// ```
///     root
///       |
///      docc1
/// ```
pub fn create_corpus_structure_simple(update: &mut GraphUpdate) {
    update
        .add_event(UpdateEvent::AddNode {
            node_name: "root".to_string(),
            node_type: "corpus".to_string(),
        })
        .unwrap();

    update
        .add_event(UpdateEvent::AddNode {
            node_name: "root/doc1".to_string(),
            node_type: "corpus".to_string(),
        })
        .unwrap();

    update
        .add_event(UpdateEvent::AddNodeLabel {
            node_name: "root/doc1".to_string(),
            anno_ns: ANNIS_NS.into(),
            anno_name: "doc".into(),
            anno_value: "doc1".into(),
        })
        .unwrap();

    update
        .add_event(UpdateEvent::AddEdge {
            source_node: "root/doc1".to_string(),
            target_node: "root".to_string(),
            layer: ANNIS_NS.to_string(),
            component_type: "PartOf".to_string(),
            component_name: "".to_string(),
        })
        .unwrap();

    update
        .add_event(UpdateEvent::AddNode {
            node_name: "root/doc1#text1".to_string(),
            node_type: "datasource".to_string(),
        })
        .unwrap();

    update
        .add_event(UpdateEvent::AddEdge {
            source_node: "root/doc1#text1".to_string(),
            target_node: "root/doc1".to_string(),
            layer: ANNIS_NS.to_string(),
            component_type: "PartOf".to_string(),
            component_name: "".to_string(),
        })
        .unwrap();
}

/// Creates éxample token objects. If a document name is given, the
/// token objects are attached to it.
///
/// The example tokens are
/// - Is (`tok1`)
/// - this (`tok2`)
/// - example (`tok3`)
/// - more (`tok4`)
/// - complicated (`tok5`)
/// - than (`tok6`)
/// - it (`tok7`)
/// - appears (`tok8`)
/// - to (`tok9`)
/// - be (`tok10`)
/// - ? (`tok11`)
///  
pub fn create_tokens(update: &mut GraphUpdate, document_node: Option<&str>) {
    let prefix = if let Some(document_node) = document_node {
        format!("{}#", document_node)
    } else {
        "".to_string()
    };

    let token_strings = [
        ("Is", " "),
        ("this", " "),
        ("example", " "),
        ("more", " "),
        ("complicated", " "),
        ("than", " "),
        ("it", " "),
        ("appears", " "),
        ("to", " "),
        ("be", ""),
        ("?", ""),
    ];
    for (i, (t, ws)) in token_strings.iter().enumerate() {
        let ws = if ws.is_empty() { None } else { Some(*ws) };
        create_token_node(
            update,
            &format!("{}tok{}", prefix, i),
            t,
            None,
            ws,
            document_node,
        );
    }

    // add the order relations
    for i in 0..token_strings.len() {
        update
            .add_event(UpdateEvent::AddEdge {
                source_node: format!("{}tok{}", prefix, i),
                target_node: format!("{}tok{}", prefix, i + 1),
                layer: ANNIS_NS.to_string(),
                component_type: "Ordering".to_string(),
                component_name: "".to_string(),
            })
            .unwrap();
    }
}

pub fn create_token_node(
    update: &mut GraphUpdate,
    node_name: &str,
    token_value: &str,
    whitespace_before: Option<&str>,
    whitespace_after: Option<&str>,
    document_node: Option<&str>,
) {
    update
        .add_event(UpdateEvent::AddNode {
            node_name: node_name.to_string(),
            node_type: "node".to_string(),
        })
        .unwrap();
    update
        .add_event(UpdateEvent::AddNodeLabel {
            node_name: node_name.to_string(),
            anno_ns: ANNIS_NS.to_string(),
            anno_name: "tok".to_string(),
            anno_value: token_value.to_string(),
        })
        .unwrap();

    if let Some(ws) = whitespace_before {
        update
            .add_event(UpdateEvent::AddNodeLabel {
                node_name: node_name.to_string(),
                anno_ns: ANNIS_NS.to_string(),
                anno_name: "tok-whitespace-before".to_string(),
                anno_value: ws.to_string(),
            })
            .unwrap();
    }
    if let Some(ws) = whitespace_after {
        update
            .add_event(UpdateEvent::AddNodeLabel {
                node_name: node_name.to_string(),
                anno_ns: ANNIS_NS.to_string(),
                anno_name: "tok-whitespace-after".to_string(),
                anno_value: ws.to_string(),
            })
            .unwrap();
    }

    if let Some(parent_node) = document_node {
        // add the token node to the document
        update
            .add_event(UpdateEvent::AddEdge {
                source_node: node_name.to_string(),
                target_node: parent_node.to_string(),
                layer: ANNIS_NS.to_string(),
                component_type: "PartOf".to_string(),
                component_name: "".to_string(),
            })
            .unwrap();
    }
}

pub fn make_span(
    update: &mut GraphUpdate,
    node_name: &str,
    covered_token_names: &[&str],
    create_source: bool,
) {
    if create_source {
        update
            .add_event(UpdateEvent::AddNode {
                node_name: node_name.to_string(),
                node_type: "node".to_string(),
            })
            .unwrap();
    }
    for c in covered_token_names {
        update
            .add_event(UpdateEvent::AddEdge {
                source_node: node_name.to_string(),
                target_node: c.to_string(),
                layer: "".to_string(),
                component_type: "Coverage".to_string(),
                component_name: "".to_string(),
            })
            .unwrap();
    }
}
