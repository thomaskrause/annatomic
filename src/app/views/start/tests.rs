use std::{path::PathBuf, sync::Arc};

use eframe::IntegrationInfo;
use egui::{mutex::RwLock, Context};
use egui_kittest::{kittest::Queryable, Harness};

#[test]
fn select_corpus() {
    let mut app_state = crate::AnnatomicApp::default();
    app_state.project.corpus_locations.insert(
        "single_sentence".to_string(),
        PathBuf::from("tests/data/single_sentence/"),
    );
    let app_state = Arc::new(RwLock::new(app_state));
    let app = |ctx: &Context| {
        let frame_info = IntegrationInfo {
            cpu_usage: Some(3.14),
        };
        let mut app_state = app_state.write();
        app_state.set_fonts(ctx);
        app_state.show(ctx, &frame_info);
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(800.0, 600.0))
        .build(app);

    harness.run();

    let corpus_selection_button = harness.get_by_label("single_sentence");
    corpus_selection_button.click();

    harness.run();
    for _ in 0..10_000 {
        harness.step();
        let app_state = app_state.read();
        if app_state.corpus_tree.is_some() {
            break;
        }
    }

    {
        let app_state = app_state.read();
        assert!(app_state.project.selected_corpus.is_some());
        assert_eq!(
            "single_sentence",
            app_state.project.selected_corpus.as_ref().unwrap().name
        );
        assert!(app_state.corpus_tree.is_some());
    }

    harness.wgpu_snapshot("select_corpus");
}
