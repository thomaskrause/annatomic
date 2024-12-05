use anyhow::Result;
use egui::{Align2, Color32, FontId, RichText, Rounding, Ui, Vec2};

use crate::AnnatomicApp;

use super::MainView;

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

pub(crate) fn select_corpus(ui: &mut Ui, app: &mut AnnatomicApp) -> Result<()> {
    let cs = app.get_corpus_storage()?;
    let corpora = cs.list()?;

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading("Select corpus");

            egui::ScrollArea::vertical().show(ui, |ui| {
                for c in corpora {
                    let is_selected = app
                        .selected_corpus
                        .as_ref()
                        .is_some_and(|selected_corpus| selected_corpus == &c.name);
                    if ui.selectable_label(is_selected, &c.name).clicked() {
                        if is_selected {
                            // Unselect the current corpus
                            app.selected_corpus = None;
                        } else {
                            // Select this corpus
                            app.selected_corpus = Some(c.name.clone());
                        }
                    }
                }
            });
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.heading("Create new corpus");
            ui.label("TOOD");
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.heading("Demo");
            if ui.link("Go to span demo").clicked() {
                app.main_view = MainView::Demo
            }
        });
    });

    Ok(())
}

pub(crate) fn demo(ui: &mut Ui, app: &mut AnnatomicApp) -> Result<()> {
    ui.heading("Annatomic Token demo");

    if ui.link("Go back to main view").clicked() {
        app.main_view = MainView::SelectCorpus;
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
                            .get_or_insert(token_rect)
                            .union(token_rect),
                    );
                    if idx > 2 && idx < 8 {
                        let offset = token_rect.translate(Vec2::new(0.0, 25.0));
                        layer2_rectangle =
                            Some(layer2_rectangle.get_or_insert(offset).union(offset));
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
                    ui.painter()
                        .rect_filled(layer2_rectangle, Rounding::ZERO, Color32::DARK_GRAY);

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

    Ok(())
}
