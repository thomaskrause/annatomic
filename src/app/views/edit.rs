use anyhow::Result;
use egui::Ui;

use crate::{app::MainView, AnnatomicApp};

pub(crate) fn show(ui: &mut Ui, app: &mut AnnatomicApp) -> Result<()> {
    ui.heading("TODO");

    if ui.link("Go back to main view").clicked() {
        app.main_view = MainView::Start;
    }
    egui::ScrollArea::horizontal().show(ui, |_ui| {
        // TODO Enumerate over all base token
    });
    Ok(())
}
