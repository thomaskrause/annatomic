use std::sync::Arc;

use egui::{accesskit::Role, mutex::RwLock, Vec2};
use egui_kittest::{
    kittest::{Key, Node, Queryable},
    Harness,
};
use graphannis::model::AnnotationComponentType;

use crate::app::set_fonts;

use super::{DocumentEditor, Editor, JobExecutor};

#[test]
fn render_token_with_labels() {
    let (graph, _config) =
        graphannis_core::graph::serialization::graphml::import::<AnnotationComponentType, _, _>(
            &include_bytes!("../../../tests/data/single_sentence.graphml")[..],
            false,
            |_| {},
        )
        .unwrap();
    let document_node = graph
        .get_node_annos()
        .get_node_id_from_name("single_sentence/zossen")
        .unwrap()
        .unwrap();
    let job = JobExecutor::default();
    let mut editor =
        DocumentEditor::create_from_graph(document_node, Arc::new(RwLock::new(graph)), job)
            .unwrap();
    let mut harness = Harness::builder().build_ui(move |ui| {
        set_fonts(ui.ctx());
        editor.show(ui);
    });
    harness.fit_contents();
    harness.run();

    harness.snapshot("render_token_with_labels");
}

#[test]
fn render_segmentation_spans() {
    let (graph, _config) =
        graphannis_core::graph::serialization::graphml::import::<AnnotationComponentType, _, _>(
            &include_bytes!("../../../tests/data/SegmentationWithGaps.graphml")[..],
            false,
            |_| {},
        )
        .unwrap();
    let document_node = graph
        .get_node_annos()
        .get_node_id_from_name("SegmentationWithGaps/doc01")
        .unwrap()
        .unwrap();
    let job = JobExecutor::default();
    let mut editor =
        DocumentEditor::create_from_graph(document_node, Arc::new(RwLock::new(graph)), job)
            .unwrap();
    let mut harness = Harness::builder().build_ui(move |ui| {
        set_fonts(ui.ctx());
        editor.show(ui);
    });
    harness.set_size(Vec2::new(900.0, 120.0));
    harness.run();

    harness.snapshot("render_segmentation_spans");
}

#[test]
fn change_segmentation_value() {
    let (graph, _config) =
        graphannis_core::graph::serialization::graphml::import::<AnnotationComponentType, _, _>(
            &include_bytes!("../../../tests/data/SegmentationWithGaps.graphml")[..],
            false,
            |_| {},
        )
        .unwrap();
    let document_node = graph
        .get_node_annos()
        .get_node_id_from_name("SegmentationWithGaps/doc01")
        .unwrap()
        .unwrap();
    let job = JobExecutor::default();
    let mut editor =
        DocumentEditor::create_from_graph(document_node, Arc::new(RwLock::new(graph)), job.clone())
            .unwrap();
    let mut harness = Harness::builder().build_ui(move |ui| {
        set_fonts(ui.ctx());
        editor.show(ui);
    });
    harness.set_size(Vec2::new(900.0, 120.0));
    harness.get_by_label("subtokenized").click();
    harness.run();
    get_text_input(&harness, "subtokenized").type_text("t");
    harness.run();
    get_text_input(&harness, "subtokenizedt").key_press(Key::Enter);
    harness.run();

    harness.snapshot("change_segmentation_value");
}

fn get_text_input<'a>(harness: &'a Harness<'_>, value: &'a str) -> Node<'a> {
    harness
        .get_all_by_value(value)
        .filter(|n| n.role() == Role::TextInput)
        .next()
        .unwrap()
}
