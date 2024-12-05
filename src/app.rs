use anyhow::Context;
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
    selected_corpus: Option<String>,
    #[serde(skip)]
    corpus_storage: Option<CorpusStorage>,
}

impl AnnatomicApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }

    fn ensure_corpus_storage_loaded(&mut self) -> anyhow::Result<()> {
        if self.corpus_storage.is_none() {
            let parent_path =
                eframe::storage_dir(APP_ID).context("Unable to get local file storage path")?;
            // Attempt to create a corpus storage and remember it
            let cs = CorpusStorage::with_auto_cache_size(&parent_path.join("db"), true)?;
            self.corpus_storage = Some(cs);
        }
        Ok(())
    }

    fn get_corpus_storage(&mut self) -> anyhow::Result<&CorpusStorage> {
       self.ensure_corpus_storage_loaded()?;
        let cs = self
            .corpus_storage
            .as_ref()
            .context("Missing corpus storage")?;
        Ok(cs)
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

        egui::CentralPanel::default().show(ctx, |ui| match self.main_view {
            MainView::SelectCorpus => views::select_corpus(ui, self),
            MainView::Demo => views::demo(ui, self),
        });
    }
}
