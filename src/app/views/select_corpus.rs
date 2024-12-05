use crate::{app::MainView, AnnatomicApp};
use anyhow::Result;
use egui::Ui;

pub(crate) fn show(ui: &mut Ui, app: &mut AnnatomicApp) -> Result<()> {
    let cs = app.get_corpus_storage()?;
    let corpora = cs.list()?;

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading("Select corpus");

            egui::ScrollArea::vertical().show(ui, |ui| {
                for c in corpora {
                    let is_selected = app
                        .selected_corpus
                        .as_ref()
                        .is_some_and(|selected_corpus| selected_corpus == &c.name);
                    if ui.selectable_label(is_selected, &c.name).clicked() {
                        if is_selected {
                            // Unselect the current corpus
                            app.selected_corpus = None;
                        } else {
                            // Select this corpus
                            app.selected_corpus = Some(c.name.clone());
                        }
                    }
                }
            });
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.heading("Create new corpus");
            ui.label("TODO");
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.heading("Demo");
            if ui.link("Go to span demo").clicked() {
                app.main_view = MainView::Demo
            }
        });
    });

    Ok(())
}
