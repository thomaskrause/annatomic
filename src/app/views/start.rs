use std::sync::Arc;

use crate::{
    app::{FgJob, MainView},
    AnnatomicApp,
};
use anyhow::{Context, Result};
use egui::{TextEdit, Ui};
use egui_notify::Toast;
use graphannis::{corpusstorage::CorpusInfo, CorpusStorage};
use log::error;
use rfd::FileDialog;

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
        import_corpus(ui, app, cs.clone());
        ui.separator();
        create_new_corpus(ui, app, cs.clone());
        ui.separator();
        demo_link(ui, app);
    });

    Ok(())
}

fn corpus_selection(ui: &mut Ui, app: &mut AnnatomicApp, corpora: &[CorpusInfo]) {
    ui.vertical(|ui| {
        ui.heading("Select");

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

fn import_corpus(ui: &mut Ui, app: &mut AnnatomicApp, cs: Arc<CorpusStorage>) {
    ui.vertical(|ui| {
        ui.heading("Import");
        if ui.button("Choose file...").clicked() {
            let dlg = FileDialog::new()
                .add_filter("GraphML (*.graphml)", &["graphml"])
                .add_filter("Zipped GraphML (*.zip)", &["zip"]);
            if let Some(path) = dlg.pick_file() {
                let job_title = format!("Importing {}", path.to_string_lossy());
                {
                    let mut job_desc = app.job_in_progress.lock();
                    *job_desc = Some(FgJob::new(&job_title));
                }
                let cloned_job_desc = app.job_in_progress.clone();
                rayon::spawn(move || {
                    let result = cs.import_from_fs(
                        &path,
                        graphannis::corpusstorage::ImportFormat::GraphML,
                        None,
                        false,
                        false,
                        |msg| {
                            let mut job_desc = cloned_job_desc.lock();
                            *job_desc = Some(FgJob::new(&job_title).msg(msg));
                        },
                    );
                    let mut job_desc = cloned_job_desc.lock();
                    if let Err(e) = result {
                        error!("{e}");
                        job_desc.replace(FgJob::new(&job_title).error_msg(e.to_string()));
                    } else {
                        *job_desc = None;
                    }
                });
            }
        }
    });
}

fn create_new_corpus(ui: &mut Ui, app: &mut AnnatomicApp, cs: Arc<CorpusStorage>) {
    ui.vertical(|ui| {
        let heading = ui.heading("Create new");
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