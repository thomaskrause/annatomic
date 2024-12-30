use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use corpus_tree::CorpusTree;
use eframe::IntegrationInfo;
use egui::{Button, Color32, FontData, Key, KeyboardShortcut, Modifiers, RichText};
use job_executor::JobExecutor;
use messages::Notifier;
use project::Project;
use serde::{Deserialize, Serialize};

mod corpus_tree;
mod job_executor;
mod messages;
mod project;
#[cfg(test)]
mod tests;
mod views;

pub(crate) const APP_ID: &str = "annatomic";
pub const QUIT_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
pub const UNDO_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
pub const REDO_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

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
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut app = if let Some(storage) = cc.storage {
            let mut app_from_storage: AnnatomicApp =
                eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app_from_storage.args = args;
            app_from_storage
        } else {
            Self {
                args,
                ..Default::default()
            }
        };
        // Set fonts once
        app.set_fonts(&cc.egui_ctx);
        // Rebuild the state that is not persisted but calculated
        app.project.load_after_init(&app.jobs)?;

        Ok(app)
    }

    pub(crate) fn set_fonts(&self, ctx: &egui::Context) {
        let mut defs = egui::FontDefinitions::default();
        // Phosphor icons
        egui_phosphor::add_to_fonts(&mut defs, egui_phosphor::Variant::Regular);

        // Icons and Emojis
        defs.font_data.insert(
            "NotoEmoji-Regular".to_owned(),
            Arc::new(FontData::from_static(include_bytes!(
                "../assets/Noto_Emoji/static/NotoEmoji-Regular.ttf"
            ))),
        );

        // Regular font
        defs.font_data.insert(
            "NotoSans-Regular".to_owned(),
            Arc::new(FontData::from_static(include_bytes!(
                "../assets/Noto_Sans/static/NotoSans-Regular.ttf"
            ))),
        );

        // Monospaced font
        defs.font_data.insert(
            "NotoSansMono-Regular".to_owned(),
            Arc::new(FontData::from_static(include_bytes!(
                "../assets/Noto_Sans_Mono/static/NotoSansMono-Regular.ttf"
            ))),
        );

        // Define the fonts to use for each font family
        defs.families.insert(
            egui::FontFamily::Proportional,
            vec![
                "NotoSans-Regular".to_owned(),
                "NotoEmoji-Regular".to_owned(),
            ],
        );
        defs.families.insert(
            egui::FontFamily::Monospace,
            vec!["NotoSansMono-Regular".to_owned()],
        );
        ctx.set_fonts(defs);
    }

    fn handle_corpus_confirmation_dialog(&mut self, ctx: &egui::Context) {
        if self.project.scheduled_for_deletion.is_some() {
            egui::Modal::new("corpus_deletion_confirmation".into()).show(ctx, |ui| {
                let corpus_name = self
                    .project
                    .scheduled_for_deletion
                    .clone()
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(egui_phosphor::regular::WARNING).color(Color32::ORANGE).size(32.0));
                    ui.label(format!("Are you sure to delete the corpus \"{corpus_name}\" permanently? This can not be undone."));
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("Do not delete the corpus.").color(Color32::BLUE))
                        .clicked()
                    {
                        self.project.scheduled_for_deletion = None;
                    }
                    ui.add_space(5.0);
                    if ui
                        .button(
                            RichText::new(format!("Delete \"{corpus_name}\" permanently"))
                                .color(Color32::RED),
                        )
                        .clicked()
                    {
                        self.project.delete_corpus(&self.jobs, corpus_name);
                    }
                });
            });
        }
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context, frame_info: &IntegrationInfo) {
        egui_extras::install_image_loaders(ctx);

        if ctx.input(|input_state| input_state.viewport().close_requested()) {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Closing application...");
                ui.label("Please wait for any background jobs to finish.");
            });
        } else {
            if ctx.input_mut(|i| i.consume_shortcut(&QUIT_SHORTCUT)) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            };

            self.handle_corpus_confirmation_dialog(ctx);
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                // The top panel is often a good place for a menu bar:

                egui::menu::bar(ui, |ui| {
                    ui.image(egui::include_image!("../assets/icon-32.png"));
                    ui.menu_button("File", |ui| {
                        if ui
                            .add(
                                Button::new("Quit")
                                    .shortcut_text(ctx.format_shortcut(&QUIT_SHORTCUT)),
                            )
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui
                            .add_enabled(
                                self.project.has_undo(),
                                Button::new("Undo")
                                    .shortcut_text(ctx.format_shortcut(&UNDO_SHORTCUT)),
                            )
                            .clicked()
                        {}
                        if ui
                            .add_enabled(
                                self.project.has_redo(),
                                Button::new("Redo")
                                    .shortcut_text(ctx.format_shortcut(&REDO_SHORTCUT)),
                            )
                            .clicked()
                        {}
                    });
                    ui.menu_button("View", |ui| {
                        egui::gui_zoom::zoom_menu_buttons(ui);
                    });
                    ui.add_space(16.0);
                    if self.args.dev {
                        if let Some(seconds) = frame_info.cpu_usage {
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
}

impl eframe::App for AnnatomicApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.show(ctx, frame.info());
    }
}
