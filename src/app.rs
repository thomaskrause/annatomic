use std::sync::Arc;

use anyhow::{Context, Result};
use corpus_tree::CorpusTree;
use egui::{Color32, RichText};
use egui_modal::Modal;
use graphannis::CorpusStorage;
use job_executor::JobExecutor;
use messages::Notifier;
use views::start::CorpusSelection;

mod corpus_tree;
mod job_executor;
mod messages;
mod views;

pub(crate) const APP_ID: &str = "annatomic";

/// Which main view to show in the app
#[derive(Default, serde::Deserialize, serde::Serialize, Clone)]
pub(crate) enum MainView {
    #[default]
    Start,
    Demo,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AnnatomicApp {
    main_view: MainView,
    corpus_selection: CorpusSelection,
    new_corpus_name: String,
    #[serde(skip)]
    corpus_tree: Option<CorpusTree>,
    #[serde(skip)]
    jobs: Arc<JobExecutor>,
    #[serde(skip)]
    notifier: Arc<Notifier>,
    #[serde(skip)]
    corpus_storage: Option<Arc<CorpusStorage>>,
}

impl AnnatomicApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self> {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            let mut app: Self = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app.ensure_corpus_storage_loaded()?;
            // Rebuild the corpus state that is not persisted but calculated
            app.schedule_corpus_tree_update();
            return Ok(app);
        }

        let mut app = Self::default();
        app.ensure_corpus_storage_loaded()?;

        Ok(app)
    }

    fn ensure_corpus_storage_loaded(&mut self) -> anyhow::Result<Arc<CorpusStorage>> {
        if let Some(cs) = &self.corpus_storage {
            Ok(cs.clone())
        } else {
            let parent_path =
                eframe::storage_dir(APP_ID).context("Unable to get local file storage path")?;
            // Attempt to create a corpus storage and remember it
            let cs = CorpusStorage::with_auto_cache_size(&parent_path.join("db"), true)?;
            let cs = Arc::new(cs);
            self.corpus_storage = Some(cs.clone());
            Ok(cs)
        }
    }

    fn handle_corpus_confirmation_dialog(&mut self, ctx: &egui::Context) {
        let modal = Modal::new(ctx, "corpus_deletion_confirmation");
        if modal.is_open() {
            modal.show(|ui| {
                let corpus_name = self
                    .corpus_selection
                    .scheduled_for_deletion
                    .clone()
                    .unwrap_or_default();
                modal.title(ui, format!("Confirm deletion of \"{corpus_name}\""));
                modal.frame(ui, |ui| {
                    modal.body_and_icon(
                        ui,
                        "Are you sure to delete the corpus permanently?",
                        egui_modal::Icon::Warning,
                    );
                });
                modal.buttons(ui, |ui| {
                    if modal
                        .button(
                            ui,
                            RichText::new("Do not delete the corpus.").color(Color32::BLUE),
                        )
                        .clicked()
                    {
                        self.corpus_selection.scheduled_for_deletion = None;
                        modal.close();
                    }
                    if modal
                        .button(
                            ui,
                            RichText::new(format!("Delete \"{corpus_name}\" permanently"))
                                .color(Color32::RED),
                        )
                        .clicked()
                    {
                        if let Some(cs) = self.corpus_storage.as_ref().cloned() {
                            let title = format!("Deleting corpus \"{corpus_name}\"");
                            self.jobs.add(
                                &title,
                                move |_job| {
                                    cs.delete(&corpus_name)?;
                                    Ok(())
                                },
                                |_result, _app| {},
                            );
                        }
                        self.corpus_selection.scheduled_for_deletion = None;
                        modal.close();
                    }
                });
            });
        }
        if self.corpus_selection.scheduled_for_deletion.is_some() {
            modal.open();
        }
    }

    fn schedule_corpus_tree_update(&mut self) {
        match self.ensure_corpus_storage_loaded() {
            Ok(cs) => {
                if let Some(corpus_name) = self.corpus_selection.name.clone() {
                    // Run a background job that creates the new corpus structure

                    let job_title = format!("Updating corpus structure for {}", &corpus_name);
                    let notifier = self.notifier.clone();
                    self.jobs.add(
                        &job_title,
                        move |_job| {
                            let corpus_tree =
                                CorpusTree::create_from_graphstorage(cs, &corpus_name, notifier)?;
                            Ok(corpus_tree)
                        },
                        |result, app| {
                            app.corpus_tree = Some(result);
                        },
                    );
                } else {
                    self.corpus_tree = None;
                }
            }
            Err(err) => {
                self.notifier.handle_error(err);
            }
        }
    }
}

impl eframe::App for AnnatomicApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        self.handle_corpus_confirmation_dialog(ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);
                if let Some(seconds) = frame.info().cpu_usage {
                    ui.label(format!("CPU usage: {:.1} ms / frame", seconds * 1000.0));
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let has_jobs = self.jobs.clone().show(ui, self);
            if !has_jobs {
                self.notifier.show(ctx);
                let response = match self.main_view {
                    MainView::Start => views::start::show(ui, self),
                    MainView::Demo => views::demo::show(ui, self),
                };
                if let Err(e) = response {
                    self.notifier.handle_error(e);
                }
            }
        });
    }
}
