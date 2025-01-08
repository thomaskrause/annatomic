use anyhow::Result;
use egui::Ui;

use crate::{app::MainView, AnnatomicApp};

pub(crate) fn show(ui: &mut Ui, app: &mut AnnatomicApp) -> Result<()> {
    if ui.link("Go back to main view").clicked() {
        app.main_view = MainView::Start;
    }

    if let Some(document_editor) = app.view_components.document_editor.get_mut() {
        document_editor.show(ui);
    }
    Ok(())
}