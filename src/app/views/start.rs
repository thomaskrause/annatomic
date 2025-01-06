use std::{fs::File, io::BufReader};

use crate::AnnatomicApp;
use anyhow::Result;
use egui::{Id, TextEdit, Ui};
use egui_notify::Toast;
use graphannis::model::AnnotationComponentType;

use rfd::FileDialog;

#[cfg(test)]
mod tests;

pub(crate) fn show(ui: &mut Ui, app: &mut AnnatomicApp) -> Result<()> {
    let corpora: Vec<_> = app.project.corpus_locations.keys().cloned().collect();

    ui.columns_const(|[c1, c2, c3, c4]| {
        if let Err(e) = corpus_selection(c1, app, &corpora) {
            app.notifier.report_error(e);
        }
        import_corpus(c2, app);
        export_corpus(c3, app);
        create_new_corpus(c4, app);
    });
    corpus_structure(ui, app);

    Ok(())
}

fn corpus_selection(ui: &mut Ui, app: &mut AnnatomicApp, corpora: &[String]) -> Result<()> {
    ui.vertical_centered(|ui| {
        ui.heading("Select");

        ui.horizontal_wrapped(|ui| {
            for c in corpora {
                let is_selected = app
                    .project
                    .selected_corpus
                    .as_ref()
                    .is_some_and(|selected_corpus| selected_corpus.name == *c);
                let label = ui.selectable_label(is_selected, c);
                label.context_menu(|ui| {
                    if ui.button("Delete").clicked() {
                        app.apply_pending_updates();
                        app.project.scheduled_for_deletion = Some(c.clone());
                    }
                });
                if label.clicked() {
                    app.apply_pending_updates();
                    if is_selected {
                        // Unselect the current corpus
                        app.project.select_corpus(&app.jobs, None);
                    } else {
                        // Select this corpus
                        app.project.select_corpus(&app.jobs, Some(c.clone()));
                    }
                }
            }
        });
    });
    Ok(())
}

fn import_corpus(ui: &mut Ui, app: &mut AnnatomicApp) {
    ui.vertical_centered(|ui| {
        ui.heading("Import");
        if ui.button("Import file...").clicked() {
            app.apply_pending_updates();
            let dlg = FileDialog::new().add_filter("GraphML (*.graphml)", &["graphml"]);
            if let Some(path) = dlg.pick_file() {
                let job_title = format!("Importing {}", path.to_string_lossy());
                let parent_dir = app.project.corpus_storage_dir();
                app.jobs.add(
                    &job_title,
                    move |job| {
                        let corpus_name = if let Some(file_name) = path.file_stem() {
                            file_name.to_string_lossy().to_string()
                        } else {
                            "UnknownCorpus".to_string()
                        };
                        let input_file = File::open(path)?;
                        let input_file_buffered = BufReader::new(input_file);
                        let (mut graph, _config_str) =
                            graphannis_core::graph::serialization::graphml::import::<
                                AnnotationComponentType,
                                _,
                                _,
                            >(input_file_buffered, false, |status| {
                                job.update_message(status);
                            })?;

                        let location = parent_dir?.join(uuid::Uuid::new_v4().to_string());
                        std::fs::create_dir_all(&location)?;

                        job.update_message("Persisting corpus");
                        graph.persist_to(&location)?;

                        Ok((corpus_name, location))
                    },
                    |(name, location), app| {
                        app.project.corpus_locations.insert(name.clone(), location);
                        app.project.select_corpus(&app.jobs, Some(name));
                    },
                );
            }
        }
    });
}

fn export_corpus(ui: &mut Ui, app: &mut AnnatomicApp) {
    ui.vertical_centered(|ui| {
        ui.heading("Export");
        if ui.button("Export file...").clicked() {
            let dlg = FileDialog::new()
                .set_can_create_directories(true)
                .add_filter("GraphML (*.graphml)", &["graphml"]);
            if let Some(path) = dlg.save_file() {
                app.project.export_to_graphml(&path, &app.jobs);
            }
        }
    });
}

fn create_new_corpus(ui: &mut Ui, app: &mut AnnatomicApp) {
    ui.vertical_centered(|ui| {
        let heading = ui.heading("Create new");
        let edit_id = Id::from("new-corpus-name");
        let edit = TextEdit::singleline(&mut app.new_corpus_name)
            .hint_text("Corpus name")
            .id(edit_id)
            .desired_width(heading.rect.width());
        ui.add(edit);

        if ui.button("Add").clicked() {
            app.apply_pending_updates();
            if app.new_corpus_name.is_empty() {
                app.notifier
                    .add_toast(Toast::warning("Empty corpus name not allowed"));
            } else if let Err(e) = app.project.new_empty_corpus(&app.new_corpus_name) {
                app.notifier.report_error(e);
            } else {
                app.notifier.add_toast(Toast::info(format!(
                    "Corpus \"{}\" added",
                    &app.new_corpus_name
                )));
                app.project
                    .select_corpus(&app.jobs, Some(app.new_corpus_name.clone()));
                app.new_corpus_name = String::new();
                ui.memory_mut(|mem| mem.surrender_focus(edit_id));
            }
        }
    });
}

fn corpus_structure(ui: &mut Ui, app: &mut AnnatomicApp) {
    if let Some(corpus_tree) = &mut app.corpus_tree {
        corpus_tree.show(ui, &app.jobs, &app.notifier);
    } else {
        ui.label("Select a corpus to edit it.");
    }
}
