use graphannis::{
    model::AnnotationComponentType,
    update::{GraphUpdate, UpdateEvent},
    AnnotationGraph,
};
use graphannis_core::graph::ANNIS_NS;
use itertools::Itertools;
use pretty_assertions::assert_eq;

use crate::app::util::example_generator;

use super::TokenHelper;

#[test]
fn example_graph_token() {
    let mut updates = GraphUpdate::new();
    example_generator::create_corpus_structure_simple(&mut updates);
    example_generator::create_tokens(&mut updates, Some("root/doc1"));
    let mut g = AnnotationGraph::with_default_graphstorages(false).unwrap();
    g.apply_update(&mut updates, |_msg| {}).unwrap();

    let token_helper = TokenHelper::new(&g).unwrap();

    let ordered_token_ids = token_helper
        .get_ordered_token("root/doc1", None)
        .unwrap()
        .into_iter()
        .map(|t_id| token_helper.spanned_text(&[t_id]).unwrap())
        .collect_vec();

    assert_eq!(
        vec![
            "Is",
            "this",
            "example",
            "more",
            "complicated",
            "than",
            "it",
            "appears",
            "to",
            "be",
            "?"
        ],
        ordered_token_ids
    );
}

#[test]
fn ordered_token_with_segmentation() {
    let mut updates = GraphUpdate::new();
    example_generator::create_corpus_structure_simple(&mut updates);
    example_generator::create_tokens(&mut updates, Some("root/doc1"));

    // Add an additional segmentation layer
    example_generator::make_span(
        &mut updates,
        "root/doc1#seg1",
        &["root/doc1#tok1", "root/doc1#tok2", "root/doc1#tok3"],
        true,
    );
    updates
        .add_event(UpdateEvent::AddNodeLabel {
            node_name: "root/doc1#seg1".into(),
            anno_ns: ANNIS_NS.into(),
            anno_name: "tok".into(),
            anno_value: "This".into(),
        })
        .unwrap();
    updates
        .add_event(UpdateEvent::AddEdge {
            source_node: "root/doc1#seg1".into(),
            target_node: "root/doc1".into(),
            layer: ANNIS_NS.into(),
            component_type: AnnotationComponentType::PartOf.to_string(),
            component_name: "".into(),
        })
        .unwrap();

    example_generator::make_span(&mut updates, "root/doc1#seg2", &["root/doc1#tok4"], true);
    updates
        .add_event(UpdateEvent::AddNodeLabel {
            node_name: "root/doc1#seg2".into(),
            anno_ns: ANNIS_NS.into(),
            anno_name: "tok".into(),
            anno_value: "more".into(),
        })
        .unwrap();
    updates
        .add_event(UpdateEvent::AddEdge {
            source_node: "root/doc1#seg2".into(),
            target_node: "root/doc1".into(),
            layer: ANNIS_NS.into(),
            component_type: AnnotationComponentType::PartOf.to_string(),
            component_name: "".into(),
        })
        .unwrap();

    example_generator::make_span(&mut updates, "root/doc1#seg3", &["root/doc1#tok5"], true);
    updates
        .add_event(UpdateEvent::AddNodeLabel {
            node_name: "root/doc1#seg3".into(),
            anno_ns: ANNIS_NS.into(),
            anno_name: "tok".into(),
            anno_value: "complicated".into(),
        })
        .unwrap();
    updates
        .add_event(UpdateEvent::AddEdge {
            source_node: "root/doc1#seg3".into(),
            target_node: "root/doc1".into(),
            layer: ANNIS_NS.into(),
            component_type: AnnotationComponentType::PartOf.to_string(),
            component_name: "".into(),
        })
        .unwrap();

    // add the order relations for the segmentation
    updates
        .add_event(UpdateEvent::AddEdge {
            source_node: "root/doc1#seg1".into(),
            target_node: "root/doc1#seg2".into(),
            layer: ANNIS_NS.to_string(),
            component_type: "Ordering".to_string(),
            component_name: "seg".to_string(),
        })
        .unwrap();
    updates
        .add_event(UpdateEvent::AddEdge {
            source_node: "root/doc1#seg2".into(),
            target_node: "root/doc1#seg3".into(),
            layer: ANNIS_NS.to_string(),
            component_type: "Ordering".to_string(),
            component_name: "seg".to_string(),
        })
        .unwrap();

    let mut g = AnnotationGraph::with_default_graphstorages(false).unwrap();
    g.apply_update(&mut updates, |_msg| {}).unwrap();

    let token_helper = TokenHelper::new(&g).unwrap();

    let ordered_token_ids = token_helper
        .get_ordered_token("root/doc1", Some("seg"))
        .unwrap()
        .into_iter()
        .map(|t_id| token_helper.spanned_text(&[t_id]).unwrap())
        .collect_vec();

    assert_eq!(vec!["This", "more", "complicated",], ordered_token_ids);
}
