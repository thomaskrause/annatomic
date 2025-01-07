use std::sync::Arc;

use egui::{mutex::RwLock, Widget};
use graphannis::AnnotationGraph;

enum TokenEditorType {
    Empty,
    BaseToken,
}
pub(crate) struct TokenEditor {
    inner_type: TokenEditorType,
    graph: Arc<RwLock<AnnotationGraph>>,
}

impl Widget for TokenEditor {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        todo!()
    }
}
