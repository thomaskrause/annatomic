use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use corpus_tree::CorpusTree;
use egui::{Color32, RichText};
use egui_modal::Modal;
use job_executor::JobExecutor;
use messages::Notifier;
use project::Project;
use serde::{Deserialize, Serialize};

mod corpus_tree;
mod job_executor;
mod messages;
mod project;
mod views;

pub(crate) const APP_ID: &str = "annatomic";

/// Which main view to show in the app
#[derive(Default, serde::Deserialize, serde::Serialize, Clone)]
pub(crate) enum MainView {
    #[default]
    Start,
    Demo,
}

#[derive(Parser, Debug, Default, Serialize, Deserialize)]
pub struct AnnatomicArgs {
    /// Start in development mode which displays additional information only relevant for developers.
    #[arg(long)]
    dev: bool,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AnnatomicApp {
    main_view: MainView,
    new_corpus_name: String,
    project: Project,
    #[serde(skip)]
    pub(crate) corpus_tree: Option<CorpusTree>,
    #[serde(skip)]
    jobs: JobExecutor,
    #[serde(skip)]
    notifier: Arc<Notifier>,
    #[serde(skip)]
    args: AnnatomicArgs,
}

impl Default for AnnatomicApp {
    fn default() -> Self {
        let notifier = Arc::new(Notifier::default());
        let project = Project::new(notifier.clone());

        Self {
            main_view: MainView::Start,
            new_corpus_name: String::default(),
            project,
            jobs: JobExecutor::default(),
            notifier,
            args: AnnatomicArgs::default(),
            corpus_tree: None,
        }
    }
}

impl AnnatomicApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, args: AnnatomicArgs) -> Result<Self> {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            let mut app: Self = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app.args = args;
            app.project.load_after_init(&app.jobs)?;
            return Ok(app);
        }

        let mut app = Self {
            args,
            ..Default::default()
        };
        app.project.load_after_init(&app.jobs)?;

        Ok(app)
    }

    fn handle_corpus_confirmation_dialog(&mut self, ctx: &egui::Context) {
        let modal = Modal::new(ctx, "corpus_deletion_confirmation");
        if modal.is_open() {
            modal.show(|ui| {
                let corpus_name = self
                    .project
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
                        self.project.scheduled_for_deletion = None;
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
                        self.project.delete_corpus(&self.jobs, corpus_name);
                        modal.close();
                    }
                });
            });
        }
        if self.project.scheduled_for_deletion.is_some() {
            modal.open();
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
        egui_extras::install_image_loaders(ctx);
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        self.handle_corpus_confirmation_dialog(ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                ui.image(egui::include_image!("../assets/icon-16.png"));
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);
                if self.args.dev {
                    if let Some(seconds) = frame.info().cpu_usage {
                        ui.label(format!("CPU usage: {:.1} ms / frame", seconds * 1000.0));
                        ui.add_space(16.0);
                    }
                }

                egui::widgets::global_theme_preference_switch(ui);
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
                    self.notifier.report_error(e);
                }
            }
        });
    }
}
