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
pub const SAVE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
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

#[derive(Default)]
enum ShutdownRequest {
    #[default]
    None,
    Requested,
    ShutdownIsSafe,
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
    shutdown_request: ShutdownRequest,
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
            shutdown_request: ShutdownRequest::None,
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
        // Phosphor icons
        egui_phosphor::add_to_fonts(&mut defs, egui_phosphor::Variant::Regular);

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

    fn apply_pending_updates(&mut self) {
        if let Some(ct) = self.corpus_tree.as_mut() {
            ct.apply_pending_updates(&self.jobs);
        }
    }

    fn has_pending_updates(&self) -> bool {
        if let Some(ct) = &self.corpus_tree {
            ct.has_pending_updates()
        } else {
            false
        }
    }

    fn consume_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&QUIT_SHORTCUT)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&UNDO_SHORTCUT)) {
            self.project.undo(&self.jobs);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&REDO_SHORTCUT)) {
            self.project.redo(&self.jobs);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SAVE_SHORTCUT)) {
            self.apply_pending_updates();
        }
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context, frame_info: &IntegrationInfo) {
        egui_extras::install_image_loaders(ctx);

        // Check if we need to react to a closing event
        if let ShutdownRequest::None = self.shutdown_request {
            if ctx.input(|input_state| input_state.viewport().close_requested()) {
                // We are currently not shutting down, so initiate the process
                if self.has_pending_updates() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    self.apply_pending_updates();
                    self.shutdown_request = ShutdownRequest::Requested;
                } else {
                    self.shutdown_request = ShutdownRequest::ShutdownIsSafe;
                }
            }
        }

        match self.shutdown_request {
            ShutdownRequest::None => {
                self.show_view(ctx, frame_info);
            }
            ShutdownRequest::Requested => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if !self.jobs.clone().show(ui, self) {
                        // No more jobs to show
                        self.shutdown_request = ShutdownRequest::ShutdownIsSafe;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            }
            ShutdownRequest::ShutdownIsSafe => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Closing application...");
                    ui.label("Please wait until the corpus is persisted to disk.");
                });
            }
        }
    }

    fn show_view(&mut self, ctx: &egui::Context, frame_info: &IntegrationInfo) {
        self.consume_shortcuts(ctx);
        self.handle_corpus_confirmation_dialog(ctx);
        let has_pending_updates = self.has_pending_updates();
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.image(egui::include_image!("../assets/icon-32.png"));
                ui.menu_button("File", |ui| {
                    if ui
                        .add_enabled(
                            has_pending_updates,
                            Button::new("Save pending changes")
                                .shortcut_text(ctx.format_shortcut(&SAVE_SHORTCUT)),
                        )
                        .clicked()
                    {
                        self.apply_pending_updates();
                    }
                    if ui
                        .add(Button::new("Quit").shortcut_text(ctx.format_shortcut(&QUIT_SHORTCUT)))
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(
                            self.project.has_undo(),
                            Button::new("Undo").shortcut_text(ctx.format_shortcut(&UNDO_SHORTCUT)),
                        )
                        .clicked()
                    {
                        self.project.undo(&self.jobs);
                    }
                    if ui
                        .add_enabled(
                            self.project.has_redo(),
                            Button::new("Redo").shortcut_text(ctx.format_shortcut(&REDO_SHORTCUT)),
                        )
                        .clicked()
                    {
                        self.project.redo(&self.jobs);
                    }
                });
                ui.menu_button("View", |ui| {
                    if self.args.dev && ui.button("Go to span demo").clicked() {
                        self.main_view = MainView::Demo
                    }
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

impl eframe::App for AnnatomicApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.show(ctx, frame.info());
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Persist the changes in the annotation graph
        self.notifier
            .report_result(self.project.persist_changes_on_exit());
    }
}
