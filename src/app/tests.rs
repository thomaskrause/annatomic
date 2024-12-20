use std::path::PathBuf;

use egui::Context;
use egui_kittest::Harness;

use super::*;

#[test]
fn show_main_page() {
    let mut state = crate::AnnatomicApp::default();
    state
        .project
        .corpus_locations
        .insert("single_sentence".to_string(), PathBuf::default());
    state
        .project
        .corpus_locations
        .insert("test".to_string(), PathBuf::default());

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
