use std::sync::Arc;

use crate::{app::MainView, AnnatomicApp};
use anyhow::{Context, Result};
use egui::{TextEdit, Ui};
use egui_notify::Toast;
use graphannis::{corpusstorage::CorpusInfo, CorpusStorage};

use rfd::FileDialog;

pub(crate) fn show(ui: &mut Ui, app: &mut AnnatomicApp) -> Result<()> {
    let cs = app
        .project
        .corpus_storage
        .as_ref()
        .context("Missing corpus storage")?
        .clone();
    let corpora = cs.list()?;

    ui.columns_const(|[c1, c2, c3, c4]| {
        if let Err(e) = corpus_selection(c1, app, &corpora) {
            app.notifier.report_error(e);
        }
        import_corpus(c2, app, cs.clone());
        create_new_corpus(c3, app, cs.clone());
        demo_link(c4, app);
    });
    corpus_structure(ui, app);

    Ok(())
}

fn corpus_selection(ui: &mut Ui, app: &mut AnnatomicApp, corpora: &[CorpusInfo]) -> Result<()> {
    ui.vertical_centered(|ui| {
        ui.heading("Select");

        ui.horizontal_wrapped(|ui| {
            for c in corpora {
                let is_selected = app
                    .project
                    .selected_corpus_name
                    .as_ref()
                    .is_some_and(|selected_corpus| selected_corpus == &c.name);
                let label = ui.selectable_label(is_selected, &c.name);
                label.context_menu(|ui| {
                    if ui.button("Delete").clicked() {
                        app.project.scheduled_for_deletion = Some(c.name.clone());
                    }
                });
                if label.clicked() {
                    if is_selected {
                        // Unselect the current corpus
                        app.project.select_corpus(&app.jobs, None);
                    } else {
                        // Select this corpus
                        app.project.select_corpus(&app.jobs, Some(c.name.clone()));
                    }
                }
            }
        });
    });
    Ok(())
}

fn import_corpus(ui: &mut Ui, app: &mut AnnatomicApp, cs: Arc<CorpusStorage>) {
    ui.vertical_centered(|ui| {
        ui.heading("Import");
        if ui.button("Choose file...").clicked() {
            let dlg = FileDialog::new()
                .add_filter("GraphML (*.graphml)", &["graphml"])
                .add_filter("Zipped GraphML (*.zip)", &["zip"]);
            if let Some(path) = dlg.pick_file() {
                let job_title = format!("Importing {}", path.to_string_lossy());
                app.jobs.add(
                    &job_title,
                    move |job| {
                        let name = cs.import_from_fs(
                            &path,
                            graphannis::corpusstorage::ImportFormat::GraphML,
                            None,
                            false,
                            false,
                            |msg| {
                                job.update_message(msg);
                            },
                        )?;
                        Ok(name)
                    },
                    |result, app| {
                        app.project.select_corpus(&app.jobs, Some(result));
                    },
                );
            }
        }
    });
}

fn create_new_corpus(ui: &mut Ui, app: &mut AnnatomicApp, cs: Arc<CorpusStorage>) {
    ui.vertical_centered(|ui| {
        let heading = ui.heading("Create new");
        let edit = TextEdit::singleline(&mut app.new_corpus_name)
            .hint_text("Corpus name")
            .desired_width(heading.rect.width());
        ui.add(edit);
        if ui.button("Add").clicked() {
            if app.new_corpus_name.is_empty() {
                app.notifier
                    .add_toast(Toast::warning("Empty corpus name not allowed"));
            } else if let Err(e) = cs.create_empty_corpus(&app.new_corpus_name, false) {
                app.notifier.report_error(e.into());
            } else {
                app.notifier.add_toast(Toast::info(format!(
                    "Corpus \"{}\" added",
                    &app.new_corpus_name
                )));
                app.project
                    .select_corpus(&app.jobs, Some(app.new_corpus_name.clone()));
                app.new_corpus_name = String::new();
            }
        }
    });
}

fn demo_link(ui: &mut Ui, app: &mut AnnatomicApp) {
    ui.vertical_centered(|ui| {
        ui.heading("Demo");
        if ui.link("Go to span demo").clicked() {
            app.main_view = MainView::Demo
        }
    });
}

fn corpus_structure(ui: &mut Ui, app: &mut AnnatomicApp) {
    if let Some(corpus_tree) = &mut app.corpus_tree {
        corpus_tree.show(ui, &mut app.project, &app.jobs);
    } else {
        ui.label("Select a corpus to edit it.");
    }
}

#[cfg(test)]
mod tests;
