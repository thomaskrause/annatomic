use std::path::Path;

use egui::Context;
use egui_kittest::Harness;
use graphannis::{corpusstorage::ImportFormat, CorpusStorage};

use super::*;

#[test]
fn show_main_page() {
    let mut state = crate::AnnatomicApp::default();
    let test_cs_db = tempfile::TempDir::new().unwrap();
    let cs = CorpusStorage::with_auto_cache_size(test_cs_db.path(), true).unwrap();
    cs.import_from_fs(
        Path::new("./tests/data/single_sentence.graphml"),
        ImportFormat::GraphML,
        Some("single_sentence".to_string()),
        true,
        true,
        |_| {},
    )
    .unwrap();
    cs.import_from_fs(
        Path::new("./tests/data/single_sentence.graphml"),
        ImportFormat::GraphML,
        Some("test".to_string()),
        true,
        true,
        |_| {},
    )
    .unwrap();
    state.project.corpus_storage = Some(Arc::new(cs));
    state.project.load_after_init(&state.jobs).unwrap();

    let app = |ctx: &Context| {
        let frame_info = IntegrationInfo {
            cpu_usage: Some(3.14),
        };
        state.show(ctx, &frame_info);
    };

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(800.0, 600.0))
        .build(app);
    harness.run();

    harness.wgpu_snapshot("show_main_page");
}
