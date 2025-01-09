use anyhow::Result;
use egui::Ui;

use crate::{app::MainView, AnnatomicApp};

pub(crate) fn show(ui: &mut Ui, app: &mut AnnatomicApp) -> Result<()> {
    if ui.link("Go back to main view").clicked() {
        app.change_view(MainView::Start);
    }

    if let Some(editor) = app.current_editor.get_mut() {
        editor.show(ui);
    }
    Ok(())
}
