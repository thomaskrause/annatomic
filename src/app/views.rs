use egui::Ui;
use graphannis::graph::NodeID;

pub(crate) mod edit;
pub(crate) mod start;

pub(crate) trait Editor: Send {
    fn show(&mut self, ui: &mut Ui);
    fn has_pending_updates(&self) -> bool;
    fn apply_pending_updates_for_editor(&mut self);
    fn get_selected_corpus_node(&self) -> Option<NodeID>;
    fn consume_shortcuts(&mut self, _ctx: &egui::Context) {}
    fn add_edit_menu_entries(&mut self, _ui: &mut egui::Ui) {}

    fn any_mut(&mut self) -> &mut dyn std::any::Any;
}
