use std::path::PathBuf;

use egui::Context;
use egui_kittest::Harness;

use super::*;

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

    let app = |ctx: &Context| {
        let frame_info = IntegrationInfo {
            cpu_usage: Some(3.14),
        };
        app_state.set_fonts(ctx);
        app_state.show(ctx, &frame_info);
    };

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(800.0, 600.0))
        .build(app);
    harness.run();

    harness.wgpu_snapshot("show_main_page");
}
