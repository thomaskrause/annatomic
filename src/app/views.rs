use egui::Ui;
use graphannis::graph::NodeID;

pub(crate) mod edit;
pub(crate) mod start;

pub(crate) trait Editor {
    fn show(&mut self, ui: &mut Ui);
    fn has_pending_updates(&self) -> bool;
    fn apply_pending_updates(&mut self);
    fn get_selected_corpus_node(&self) -> Option<NodeID>;
}
