use std::sync::Arc;

use anyhow::{Context, Result};
use egui_notify::Toasts;
use graphannis::CorpusStorage;

mod views;

pub(crate) const APP_ID: &str = "annatomic";

/// Which main view to show in the app
#[derive(Default, serde::Deserialize, serde::Serialize, Clone)]
pub(crate) enum MainView {
    #[default]
    SelectCorpus,
    Demo,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AnnatomicApp {
    main_view: MainView,
    #[serde(skip)]
    messages: Toasts,
    selected_corpus: Option<String>,
    new_corpus_name: String,
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
            return Ok(app);
        }

        let mut app = Self::default();
        app.ensure_corpus_storage_loaded()?;
        Ok(app)
    }

    fn ensure_corpus_storage_loaded(&mut self) -> anyhow::Result<()> {
        if self.corpus_storage.is_none() {
            let parent_path =
                eframe::storage_dir(APP_ID).context("Unable to get local file storage path")?;
            // Attempt to create a corpus storage and remember it
            let cs = CorpusStorage::with_auto_cache_size(&parent_path.join("db"), true)?;
            self.corpus_storage = Some(Arc::new(cs));
        }
        Ok(())
    }
}

impl eframe::App for AnnatomicApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.messages.show(ctx);
            match self.main_view {
                MainView::SelectCorpus => views::select_corpus::show(ui, self),
                MainView::Demo => views::demo::show(ui, self),
            }
        });
    }
}
