use egui::{Align2, Color32, FontId, RichText, Rounding, Ui, Vec2};

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub(crate) enum MainView {
    ListCorpora { selected: Option<String> },
    Demo { selected: String },
}

impl Default for MainView {
    fn default() -> Self {
        Self::ListCorpora { selected: None }
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

impl MainView {
    pub(crate) fn show(&mut self, ui: &mut Ui) -> Self {
        let result = match self {
            MainView::ListCorpora { .. } => self.show_corpus_list(ui),
            MainView::Demo { .. } => self.show_demo(ui),
        };
        result
    }

    fn get_selected_corpus(&self) -> Option<&str> {
        match self {
            MainView::ListCorpora { selected } => selected.as_ref().map(|s| s.as_str()),
            MainView::Demo { selected } => Some(&selected),
        }
    }

    fn show_corpus_list(&self, ui: &mut Ui) -> Self {
        ui.heading("Select corpus");

        let selected = self.get_selected_corpus().unwrap_or_default();
        if ui.button("Span demo").clicked() {
            MainView::Demo {
                selected: selected.to_string(),
            }
        } else {
            self.clone()
        }
    }

    fn show_demo(&self, ui: &mut Ui) -> Self {
        ui.heading("Annatomic Token demo");

        if ui.button("List corpora").clicked() {
            return MainView::ListCorpora {
                selected: self.get_selected_corpus().map(|s| s.to_string()),
            };
        }

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                for sent_nr in 0..NUMBER_OF_SENTENCES {
                    let mut sentence_rectangle = None;
                    let mut layer2_rectangle = None;
                    for (idx, t) in EXAMPLE_SENTENCE.iter().enumerate() {
                        let token_rect = ui
                            .label(RichText::new(*t).font(FontId::proportional(14.0)))
                            .rect
                            .translate(Vec2::new(0.0, 20.0));

                        sentence_rectangle = Some(
                            sentence_rectangle
                                .get_or_insert_with(|| token_rect)
                                .union(token_rect),
                        );
                        if idx > 2 && idx < 8 {
                            let offset = token_rect.translate(Vec2::new(0.0, 25.0));
                            layer2_rectangle =
                                Some(layer2_rectangle.get_or_insert_with(|| offset).union(offset));
                        }
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
                    if let Some(layer2_rectangle) = layer2_rectangle {
                        ui.painter().rect_filled(
                            layer2_rectangle,
                            Rounding::ZERO,
                            Color32::DARK_GRAY,
                        );

                        ui.painter().text(
                            layer2_rectangle.center(),
                            Align2::CENTER_CENTER,
                            format!("Layer 2 - {sent_nr}"),
                            FontId::proportional(14.0),
                            Color32::WHITE,
                        );
                    }
                }
            });
            ui.add_space(80.0);
        });

        ui.add_space(16.0);

        self.clone()
    }
}
