use egui::Ui;

use super::{demo::DemoView, MainView};

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub(crate) struct CorpusListView {
    selected: Option<String>,
}

impl CorpusListView {
    pub(super) fn new(selected: Option<String>) -> Self {
        Self { selected }
    }
    pub(super) fn show(&self, ui: &mut Ui) -> Option<MainView> {
        ui.heading("Select corpus");

        if ui.button("Span demo").clicked() {
            let selected = self
                .selected
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or_default();
            Some(MainView::Demo(DemoView::new(selected.to_string())))
        } else {
            None
        }
    }
}
