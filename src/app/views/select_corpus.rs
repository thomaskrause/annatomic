use std::sync::Arc;

use crate::{app::MainView, AnnatomicApp};
use anyhow::{Context, Result};
use egui::{TextEdit, Ui};
use egui_notify::Toast;
use graphannis::{corpusstorage::CorpusInfo, CorpusStorage};

#[derive(Default, serde::Deserialize, serde::Serialize, Clone)]

pub(crate) struct CorpusSelection {
    pub(crate) name: Option<String>,
    pub(crate) scheduled_for_deletion: Option<String>,
}

pub(crate) fn show(ui: &mut Ui, app: &mut AnnatomicApp) -> Result<()> {
    let cs = app
        .corpus_storage
        .as_ref()
        .context("Missing corpus storage")?
        .clone();
    let corpora = cs.list()?;

    ui.horizontal(|ui| {
        corpus_selection(ui, app, &corpora);
        ui.separator();
        create_new_corpus(ui, app, cs.clone());
        ui.separator();
        demo_link(ui, app);
    });

    Ok(())
}

fn corpus_selection(ui: &mut Ui, app: &mut AnnatomicApp, corpora: &[CorpusInfo]) {
    ui.vertical(|ui| {
        ui.heading("Select corpus");

        egui::ScrollArea::vertical().show(ui, |ui| {
            for c in corpora {
                let is_selected = app
                    .corpus_selection
                    .name
                    .as_ref()
                    .is_some_and(|selected_corpus| selected_corpus == &c.name);
                let label = ui.selectable_label(is_selected, &c.name);
                label.context_menu(|ui| {
                    if ui.button("Delete").clicked() {
                        app.corpus_selection.scheduled_for_deletion = Some(c.name.clone());
                    }
                });
                if label.clicked() {
                    if is_selected {
                        // Unselect the current corpus
                        app.corpus_selection.name = None;
                    } else {
                        // Select this corpus
                        app.corpus_selection.name = Some(c.name.clone());
                    }
                }
            }
        });
    });
}

fn create_new_corpus(ui: &mut Ui, app: &mut AnnatomicApp, cs: Arc<CorpusStorage>) {
    ui.vertical(|ui| {
        let heading = ui.heading("Create new corpus");
        let edit = TextEdit::singleline(&mut app.new_corpus_name)
            .hint_text("Corpus name")
            .desired_width(heading.rect.width());
        ui.add(edit);
        if ui.button("Add").clicked() {
            if app.new_corpus_name.is_empty() {
                app.messages
                    .add(Toast::warning("Empty corpus name not allowed"));
            } else if let Err(e) = cs.create_empty_corpus(&app.new_corpus_name, false) {
                app.messages.add(Toast::error(format!("{e}")));
            } else {
                app.messages.add(Toast::info(format!(
                    "Corpus \"{}\" added",
                    &app.new_corpus_name
                )));
                app.corpus_selection.name = Some(app.new_corpus_name.to_string());
                app.new_corpus_name = String::new();
            }
        }
    });
}

fn demo_link(ui: &mut Ui, app: &mut AnnatomicApp) {
    ui.vertical(|ui| {
        ui.heading("Demo");
        if ui.link("Go to span demo").clicked() {
            app.main_view = MainView::Demo
        }
    });
}
