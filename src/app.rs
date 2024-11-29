use egui::{Align2, Color32, FontId, RichText, Rounding, Vec2};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct AnnatomicApp {
    // Example stuff:
    label: String,

    #[serde(skip)] // This how you opt-out of serialization of a field
    value: f32,
}

impl Default for AnnatomicApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
        }
    }
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
}

const NUMBER_OF_SENTENCES: usize = 5_000;

const EXAMPLE_SENTENCE: [&str; 11] = [
    "Is",
    "this",
    "example",
    "more",
    "complicated",
    "than",
    "it",
    "needs",
    "to",
    "be",
    "?",
];

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
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("Annatomic Token demo");

            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    for sent_nr in 0..NUMBER_OF_SENTENCES {
                        let mut sentence_rectangle = None;
                        for t in EXAMPLE_SENTENCE {
                            let token_rect = ui
                                .label(RichText::new(t).font(FontId::proportional(14.0)))
                                .rect
                                .translate(Vec2::new(0.0, 20.0));

                            sentence_rectangle = Some(
                                sentence_rectangle
                                    .get_or_insert_with(|| token_rect)
                                    .union(token_rect),
                            );
                        }
                        if let Some(sentence_rectangle) = sentence_rectangle {
                            ui.painter().rect_filled(
                                sentence_rectangle,
                                Rounding::ZERO,
                                Color32::DARK_GRAY,
                            );

                            ui.painter().text(
                                sentence_rectangle.center(),
                                Align2::CENTER_CENTER,
                                format!("Sentence {sent_nr}"),
                                FontId::proportional(14.0),
                                Color32::WHITE,
                            );
                        }
                    }
                });
                ui.add_space(30.0);
            });

            ui.add_space(16.0);
        });
    }
}
