use std::sync::Arc;

use egui::{mutex::RwLock, Widget};
use graphannis::{graph::NodeID, AnnotationGraph};

pub(crate) struct TokenEditor {
    node_id: NodeID,
    graph: Arc<RwLock<AnnotationGraph>>,
}

impl Widget for TokenEditor {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        todo!()
    }
}
