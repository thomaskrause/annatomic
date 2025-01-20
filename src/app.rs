use std::sync::{Arc, OnceLock};

use anyhow::Result;
use clap::Parser;
use corpus_tree::CorpusTree;
use editors::DocumentEditor;
use eframe::IntegrationInfo;
use egui::{Button, Color32, FontData, Key, KeyboardShortcut, Modifiers, RichText, Theme};
use graphannis::graph::NodeID;
use job_executor::JobExecutor;
use messages::Notifier;
use project::Project;
use serde::{Deserialize, Serialize};
use views::Editor;

mod corpus_tree;
mod editors;
pub(crate) mod job_executor;
mod messages;
mod project;
#[cfg(test)]
mod tests;
pub(crate) mod util;
mod views;

pub(crate) const APP_ID: &str = "annatomic";
pub const QUIT_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
pub const SAVE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
pub const UNDO_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
pub const REDO_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

pub const CHANGE_PENDING_COLOR_DARK: Color32 = Color32::from_rgb(160, 50, 50);
pub const CHANGE_PENDING_COLOR_LIGHT: Color32 = Color32::from_rgb(255, 128, 128);

/// Which main view to show in the app
#[derive(Default, serde::Deserialize, serde::Serialize, Clone, PartialEq)]
pub(crate) enum MainView {
    #[default]
    Start,
    EditDocument {
        node_id: NodeID,
    },
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
    current_editor: OnceLock<Box<dyn Editor>>,
    #[serde(skip)]
    shutdown_request: ShutdownRequest,
    #[serde(skip)]
    jobs: JobExecutor,
    #[serde(skip)]
    notifier: Notifier,
    #[serde(skip)]
    args: AnnatomicArgs,
}

impl Default for AnnatomicApp {
    fn default() -> Self {
        let notifier = Notifier::default();
        let jobs = JobExecutor::default();
        let project = Project::new(notifier.clone(), jobs.clone());

        Self {
            main_view: MainView::Start,
            new_corpus_name: String::default(),
            project,
            jobs,
            notifier,
            args: AnnatomicArgs::default(),
            current_editor: OnceLock::new(),
            shutdown_request: ShutdownRequest::None,
        }
    }
}

pub(crate) fn set_fonts(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();

    // Symbols and Emojis
    defs.font_data.insert(
        "NotoEmoji-Regular".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Noto_Emoji/static/NotoEmoji-Regular.ttf"
        ))),
    );
    defs.font_data.insert(
        "NotoSansMath-Regular".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Noto_Sans_Math/NotoSansMath-Regular.ttf"
        ))),
    );
    defs.font_data.insert(
        "NotoSansSymbols2-Regular".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Noto_Sans_Symbols_2/NotoSansSymbols2-Regular.ttf"
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
            "NotoSansMath-Regular".to_owned(),
            "NotoSansSymbols2-Regular".to_owned(),
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
        set_fonts(&cc.egui_ctx);
        // Rebuild the state that is not persisted but calculated
        app.project
            .load_after_init(app.notifier.clone(), app.jobs.clone())?;
        Ok(app)
    }

    pub(crate) fn change_view(&mut self, new_view: MainView) {
        if self.main_view != new_view {
            self.main_view = new_view;
            self.load_editor(true);
        }
    }

    pub(crate) fn load_editor(&mut self, force_refresh: bool) {
        let selected_corpus_node = {
            self.current_editor
                .get()
                .and_then(|editor| editor.get_selected_corpus_node())
        };
        match self.main_view {
            MainView::Start => {
                if let Some(corpus) = &self.project.selected_corpus {
                    let job_title = "Creating corpus tree editor";

                    let needs_refresh = force_refresh || self.current_editor.get().is_none();
                    if needs_refresh && !self.jobs.has_active_job_with_title(job_title) {
                        self.current_editor = OnceLock::new();

                        let corpus_cache = self.project.corpus_cache.clone();
                        let jobs = self.jobs.clone();
                        let notifier = self.notifier.clone();
                        let location = corpus.location.clone();
                        self.jobs.add(
                            job_title,
                            move |_| {
                                let graph = corpus_cache.get(&location)?;
                                let corpus_tree = CorpusTree::create_from_graph(
                                    graph,
                                    selected_corpus_node,
                                    jobs,
                                    notifier,
                                )?;
                                Ok(corpus_tree)
                            },
                            |corpus_tree, app| {
                                app.current_editor.get_or_init(|| Box::new(corpus_tree));
                            },
                        );
                    }
                } else {
                    self.current_editor = OnceLock::new();
                }
            }
            MainView::EditDocument { node_id } => {
                if let Some(corpus) = &self.project.selected_corpus {
                    let job_title = "Creating document editor";
                    let needs_refresh = force_refresh || self.current_editor.get().is_none();
                    if needs_refresh && !self.jobs.has_active_job_with_title(job_title) {
                        self.current_editor = OnceLock::new();
                        let corpus_cache = self.project.corpus_cache.clone();
                        let location = corpus.location.clone();
                        let jobs = self.jobs.clone();
                        self.jobs.add(
                            job_title,
                            move |_| {
                                let graph = corpus_cache.get(&location)?;
                                let document_editor =
                                    DocumentEditor::create_from_graph(node_id, graph, jobs)?;

                                Ok(document_editor)
                            },
                            |document_editor, app| {
                                app.current_editor.get_or_init(|| Box::new(document_editor));
                            },
                        );
                    }
                }
            }
        }
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
                        self.project.delete_corpus(corpus_name);
                    }
                });
            });
        }
    }

    pub(crate) fn select_corpus(&mut self, selection: Option<String>) {
        self.project.select_corpus(selection);
        self.load_editor(true);
    }

    fn apply_pending_updates(&mut self) {
        if let Some(editor) = self.current_editor.get_mut() {
            editor.apply_pending_updates();
        }
    }

    fn has_pending_updates(&self) -> bool {
        if let Some(editor) = self.current_editor.get() {
            editor.has_pending_updates()
        } else {
            false
        }
    }

    fn consume_shortcuts(&mut self, ctx: &egui::Context) {
        // Consume any potential context sensitve shortcuts from the editor
        if let Some(editor) = self.current_editor.get_mut() {
            editor.consume_shortcuts(ctx);
        }

        // Consume all shortcuts from the application itself, which can be active at any time
        if ctx.input_mut(|i| i.consume_shortcut(&QUIT_SHORTCUT)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&UNDO_SHORTCUT)) {
            self.project.undo();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&REDO_SHORTCUT)) {
            self.project.redo();
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
                            Button::new("Apply pending changes immediately")
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
                        self.project.undo();
                    }
                    if ui
                        .add_enabled(
                            self.project.has_redo(),
                            Button::new("Redo").shortcut_text(ctx.format_shortcut(&REDO_SHORTCUT)),
                        )
                        .clicked()
                    {
                        self.project.redo();
                    }
                });
                ui.menu_button("View", |ui| {
                    egui::gui_zoom::zoom_menu_buttons(ui);
                });
                ui.add_space(16.0);
                ui.separator();
                let marker_color = if ui.ctx().theme() == Theme::Light {
                    CHANGE_PENDING_COLOR_LIGHT
                } else {
                    CHANGE_PENDING_COLOR_DARK
                };
                if self.has_pending_updates() {
                    ui.label(RichText::new("Has pending changes").color(marker_color));
                } else {
                    ui.label("No pending changes");
                }
                ui.separator();
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
                    MainView::EditDocument { .. } => views::edit::show(ui, self),
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
        self.load_editor(false);
        self.show(ctx, frame.info());
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Persist the changes in the annotation graph
        self.notifier
            .report_result(self.project.persist_changes_on_exit());
    }
}
