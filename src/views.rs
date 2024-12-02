use corpus_list::CorpusListView;
use demo::DemoView;
use egui::Ui;

mod corpus_list;
mod demo;

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub(crate) enum MainView {
    ListCorpora(CorpusListView),
    Demo(DemoView),
}

impl Default for MainView {
    fn default() -> Self {
        Self::ListCorpora(CorpusListView::default())
    }
}

impl MainView {
    pub(crate) fn show(&mut self, ui: &mut Ui) -> Option<Self> {
        let result = match self {
            MainView::ListCorpora(view) => view.show(ui),
            MainView::Demo(view) => view.show(ui),
        };
        result
    }
}
