#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use annatomic::{AnnatomicApp, AnnatomicArgs};
use clap::Parser;

fn main() -> eframe::Result {
    let args = AnnatomicArgs::parse();

    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_app_id("annatomic")
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "annatomic",
        native_options,
        Box::new(|cc| {
            let app = AnnatomicApp::new(cc, args)?;
            Ok(Box::new(app))
        }),
    )
}
