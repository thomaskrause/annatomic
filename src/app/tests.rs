use std::{io::Read, path::PathBuf};

use egui::{mutex::RwLock, Context};
use egui_kittest::Harness;
use graphannis::model::AnnotationComponentType;
use tempfile::TempDir;

use super::*;

pub(crate) fn create_app_with_corpus<R: Read, S: Into<String>>(
    corpus_name: S,
    graphml: R,
) -> crate::AnnatomicApp {
    // Load the graphml into a temporary folder
    let (mut graph, _config) = graphannis_core::graph::serialization::graphml::import::<
        AnnotationComponentType,
        _,
        _,
    >(graphml, false, |_| {})
    .unwrap();
    let graph_dir = TempDir::new().unwrap();
    graph.persist_to(graph_dir.path()).unwrap();

    let mut app_state = crate::AnnatomicApp::default();
    app_state
        .project
        .corpus_locations
        .insert(corpus_name.into(), graph_dir.into_path());
    app_state
}

pub(crate) fn create_test_harness(
    app_state: crate::AnnatomicApp,
) -> (Harness<'static>, Arc<RwLock<crate::AnnatomicApp>>) {
    let app_state = Arc::new(RwLock::new(app_state));
    let result_app_state = app_state.clone();
    let app = move |ctx: &Context| {
        let frame_info = IntegrationInfo {
            cpu_usage: Some(3.14),
        };
        let mut app_state = app_state.write();
        app_state.set_fonts(ctx);
        app_state.show(ctx, &frame_info);
    };

    let harness = Harness::builder()
        .with_size(egui::Vec2::new(800.0, 600.0))
        .build(app);

    (harness, result_app_state.clone())
}

#[test]
fn show_main_page() {
    let mut app_state = crate::AnnatomicApp::default();

    app_state
        .project
        .corpus_locations
        .insert("single_sentence".to_string(), PathBuf::default());
    app_state
        .project
        .corpus_locations
        .insert("test".to_string(), PathBuf::default());

    let (mut harness, _) = create_test_harness(app_state);
    harness.run();

    harness.wgpu_snapshot("show_main_page");
}
