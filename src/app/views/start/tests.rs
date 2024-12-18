use std::path::Path;

use egui::{CentralPanel, Context};
use egui_kittest::Harness;
use graphannis::corpusstorage::ImportFormat;

use super::*;

#[test]
fn start_view() {
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
        CentralPanel::default().show(ctx, |ui| {
            show(ui, &mut state).unwrap();
        });
    };

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(800.0, 600.0))
        .build(app);
    harness.run();

    harness.wgpu_snapshot("start_view");
}
