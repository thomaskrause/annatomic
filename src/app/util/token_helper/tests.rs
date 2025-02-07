use graphannis::{graph::NodeID, update::GraphUpdate, AnnotationGraph};
use graphannis_core::graph::NODE_NAME_KEY;
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
    example_generator::create_segmentation(&mut updates);

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

#[test]
fn token_before_and_after_base() {
    let mut updates = GraphUpdate::new();
    example_generator::create_corpus_structure_simple(&mut updates);
    example_generator::create_tokens(&mut updates, Some("root/doc1"));
    // Create a span we will use to the the token before and after
    example_generator::make_span(
        &mut updates,
        "root/doc1#span",
        &["root/doc1#tok3", "root/doc1#tok4"],
        true,
    );
    let mut g = AnnotationGraph::with_default_graphstorages(false).unwrap();
    g.apply_update(&mut updates, |_msg| {}).unwrap();

    let span_id = g
        .get_node_annos()
        .get_node_id_from_name("root/doc1#span")
        .unwrap()
        .unwrap();

    let token_helper = TokenHelper::new(&g).unwrap();

    // Get an node and find the token before and after another token
    let tok3_id = g
        .get_node_annos()
        .get_node_id_from_name("root/doc1#tok3")
        .unwrap()
        .unwrap();
    assert_eq!(
        "root/doc1#tok2",
        node_name(
            token_helper
                .get_token_before(tok3_id, None)
                .unwrap()
                .unwrap(),
            &g
        )
    );
    assert_eq!(
        "root/doc1#tok4",
        node_name(
            token_helper
                .get_token_after(tok3_id, None)
                .unwrap()
                .unwrap(),
            &g
        )
    );

    // Get an node and find the token before and after the span
    assert_eq!(
        "root/doc1#tok2",
        node_name(
            token_helper
                .get_token_before(span_id, None)
                .unwrap()
                .unwrap(),
            &g
        )
    );
    assert_eq!(
        "root/doc1#tok5",
        node_name(
            token_helper
                .get_token_after(span_id, None)
                .unwrap()
                .unwrap(),
            &g
        )
    );
}

#[test]
fn token_before_and_after_segmentation() {
    let mut updates = GraphUpdate::new();
    example_generator::create_corpus_structure_simple(&mut updates);
    example_generator::create_tokens(&mut updates, Some("root/doc1"));
    example_generator::create_segmentation(&mut updates);

    let mut g = AnnotationGraph::with_default_graphstorages(false).unwrap();
    g.apply_update(&mut updates, |_msg| {}).unwrap();

    let token_helper = TokenHelper::new(&g).unwrap();

    // Get an node and find the token before and after another token
    let seg2_id = g
        .get_node_annos()
        .get_node_id_from_name("root/doc1#seg2")
        .unwrap()
        .unwrap();

    assert_eq!(
        "root/doc1#seg1",
        node_name(
            token_helper
                .get_token_before(seg2_id, Some("seg"))
                .unwrap()
                .unwrap(),
            &g
        )
    );
    assert_eq!(
        "root/doc1#seg3",
        node_name(
            token_helper
                .get_token_after(seg2_id, Some("seg"))
                .unwrap()
                .unwrap(),
            &g
        )
    );
}

fn node_name(id: NodeID, g: &AnnotationGraph) -> String {
    g.get_node_annos()
        .get_value_for_item(&id, &NODE_NAME_KEY)
        .unwrap()
        .unwrap()
        .to_string()
}
