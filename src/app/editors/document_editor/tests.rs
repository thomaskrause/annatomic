use std::sync::Arc;

use anyhow::Context;
use egui::{accesskit::Role, mutex::RwLock, Vec2};
use egui_kittest::{
    kittest::{Key, Node, Queryable},
    Harness,
};
use graphannis::model::AnnotationComponentType;

use crate::{
    app::{
        set_fonts,
        tests::{
            create_app_with_corpus, create_test_harness, wait_for_editor, wait_until_jobs_finished,
        },
    },
    assert_screenshots,
};

use super::{DocumentEditor, Editor, JobExecutor};

fn create_example_ui(
    graphml: &[u8],
    document_node: &str,
) -> (Harness<'static>, Arc<RwLock<DocumentEditor>>) {
    let (graph, _config) = graphannis_core::graph::serialization::graphml::import::<
        AnnotationComponentType,
        _,
        _,
    >(graphml, false, |_| {})
    .unwrap();
    let document_node = graph
        .get_node_annos()
        .get_node_id_from_name(document_node)
        .unwrap()
        .unwrap();
    let job = JobExecutor::default();
    let editor =
        DocumentEditor::create_from_graph(document_node, Arc::new(RwLock::new(graph)), job.clone())
            .unwrap();
    let editor = Arc::new(RwLock::new(editor));
    let editor_for_closure = editor.clone();
    let mut harness = Harness::builder().build_ui(move |ui| {
        set_fonts(ui.ctx());
        let mut editor = editor_for_closure.write();
        editor.show(ui);
    });
    harness.fit_contents();

    (harness, editor)
}

#[test]
fn render_token_with_labels() {
    let (mut harness, _) = create_example_ui(
        &include_bytes!("../../../../tests/data/single_sentence.graphml")[..],
        "single_sentence/zossen",
    );
    harness.run();
    harness.snapshot("render_token_with_labels");
}

#[test]
fn select_token_range() {
    let (mut harness, editor) = create_example_ui(
        &include_bytes!("../../../../tests/data/single_sentence.graphml")[..],
        "single_sentence/zossen",
    );
    harness
        .get_by_label_contains("Token ranging from 1 to 1")
        .click();
    harness.run();
    assert_eq!(1, editor.read().selected_nodes.len());
    let r1 = harness.try_snapshot("select_token_range_first_token");

    // Emulate pressing and holding the shift key
    editor.write().select_range(5);
    harness.run();
    assert_eq!(5, editor.read().selected_nodes.len());

    let r2 = harness.try_snapshot("select_token_range");

    assert_screenshots![r1, r2];
}

#[test]
fn render_segmentation_spans() {
    let (mut harness, _) = create_example_ui(
        &include_bytes!("../../../../tests/data/SegmentationWithGaps.graphml")[..],
        "SegmentationWithGaps/doc01",
    );
    harness.set_size(Vec2::new(2100.0, 210.0));
    harness.run();

    harness.snapshot("render_segmentation_spans");
}

#[test]
fn change_segmentation_value() {
    let (mut harness, editor) = create_example_ui(
        &include_bytes!("../../../../tests/data/SegmentationWithGaps.graphml")[..],
        "SegmentationWithGaps/doc01",
    );
    harness.set_size(Vec2::new(2100.0, 210.0));
    harness.run();
    // No node should be selected
    assert_eq!(0, editor.read().selected_nodes.len());
    // First click is selection
    harness
        .get_by_label_contains("Token ranging from 7 to 8")
        .click();
    harness.run();
    assert_eq!(1, editor.read().selected_nodes.len());
    // Second click to activate editing
    harness
        .get_by_label_contains("Selected token ranging from 7 to 8")
        .click();
    harness.run();

    get_text_input(&harness, "subtokenized").type_text("t");
    harness.run();
    get_text_input(&harness, "subtokenizedt").key_press(Key::Enter);
    harness.run();

    harness.snapshot("change_segmentation_value");
}

#[test]
fn delete_and_add_segmentation() {
    let app_state = create_app_with_corpus(
        "SegmentationWithGaps",
        &include_bytes!("../../../../tests/data/SegmentationWithGaps.graphml")[..],
    );
    let (mut harness, app_state) = create_test_harness(app_state);
    harness.set_size(Vec2::new(1200.0, 600.0));
    harness.run();

    // Open the document editor
    harness.get_by_label("SegmentationWithGaps").click();
    wait_for_editor(&mut harness, app_state.clone());
    harness.get_by_label("SegmentationWithGaps/doc01").click();
    harness.run();
    harness.get_by_label("Open selected in editor").click();
    harness.run();
    wait_for_editor(&mut harness, app_state.clone());

    // Manually select the nodes to delete
    {
        let mut app_state = app_state.write();
        let editor = app_state
            .current_editor
            .get_mut()
            .unwrap()
            .any_mut()
            .downcast_mut::<DocumentEditor>()
            .unwrap();
        editor.selected_nodes.clear();
        // example
        editor
            .selected_nodes
            .insert("SegmentationWithGaps/doc01#sSpan32".to_string());
        // of
        editor
            .selected_nodes
            .insert("SegmentationWithGaps/doc01#sSpan33".to_string());
        // a
        editor
            .selected_nodes
            .insert("SegmentationWithGaps/doc01#sSpan34".to_string());
        // ſub⸗
        editor
            .selected_nodes
            .insert("SegmentationWithGaps/doc01#sSpan35".to_string());

        editor.delete_selected_nodes();
    }
    wait_until_jobs_finished(&mut harness, app_state.clone());
    {
        let mut app_state = app_state.write();
        let editor = app_state
            .current_editor
            .get_mut()
            .unwrap()
            .any_mut()
            .downcast_mut::<DocumentEditor>()
            .unwrap();
        // Select the token that the new segmentation node should cover
        editor.selected_nodes.clear();
        editor
            .selected_nodes
            .insert("SegmentationWithGaps/doc01#tok_6".to_string());
        editor
            .selected_nodes
            .insert("SegmentationWithGaps/doc01#tok_7".to_string());
        // Trigger adding the segmentation node
        editor.add_segmentation_for_selection(1);
    }
    wait_until_jobs_finished(&mut harness, app_state.clone());
    harness.run();

    harness.snapshot("delete_and_add_segmentation");
}

fn get_text_input<'a>(harness: &'a Harness<'_>, value: &'a str) -> Node<'a> {
    harness
        .get_all_by_value(value)
        .filter(|n| n.role() == Role::TextInput)
        .next()
        .context(format!("Missing text input with value \"{value}\""))
        .unwrap()
}
