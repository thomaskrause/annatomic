use std::sync::Arc;

use egui::mutex::RwLock;
use egui_kittest::Harness;
use graphannis::model::AnnotationComponentType;

use crate::app::set_fonts;

use super::{DocumentEditor, Editor, Notifier};

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
    let notifier = Notifier::default();
    let mut editor =
        DocumentEditor::create_from_graph(document_node, Arc::new(RwLock::new(graph)), notifier)
            .unwrap();
    let mut harness = Harness::builder().build_ui(move |ui| {
        set_fonts(ui.ctx());
        editor.show(ui);
    });
    harness.fit_contents();
    harness.run();

    harness.wgpu_snapshot("render_token_with_labels");
}
