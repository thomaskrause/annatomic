use egui::Ui;
use graphannis::graph::NodeID;

use super::{AnnatomicApp, CorpusTree, DocumentEditor};
use std::sync::OnceLock;
pub(crate) mod demo;
pub(crate) mod edit;
pub(crate) mod start;

pub(crate) trait Editor {
    fn show(&mut self, ui: &mut Ui);
    fn has_pending_updates(&self) -> bool;
    fn apply_pending_updates(&mut self);
    fn get_selected_corpus_node(&self) -> Option<NodeID>;
}

pub(crate) fn load_components_for_view(app: &mut AnnatomicApp, force_refresh: bool) {
    let selected_corpus_node = {
        app.current_editor
            .get()
            .and_then(|editor| editor.get_selected_corpus_node())
    };
    match app.main_view {
        super::MainView::Start => {
            if let Some(corpus) = &app.project.selected_corpus {
                let job_title = "Creating corpus tree editor";

                let needs_refresh = force_refresh || app.current_editor.get().is_none();
                if needs_refresh && !app.jobs.has_active_job_with_title(job_title) {
                    app.current_editor = OnceLock::new();

                    let corpus_cache = app.project.corpus_cache.clone();
                    let jobs = app.jobs.clone();
                    let notifier = app.notifier.clone();
                    let location = corpus.location.clone();
                    app.jobs.add(
                        job_title,
                        move |_| {
                            let graph = corpus_cache.get(&location)?;
                            let corpus_tree = CorpusTree::create_from_graph(
                                graph,
                                selected_corpus_node,
                                jobs,
                                notifier,
                            )?;
                            Ok(corpus_tree)
                        },
                        |corpus_tree, app| {
                            app.current_editor.get_or_init(|| Box::new(corpus_tree));
                        },
                    );
                }
            } else {
                app.current_editor = OnceLock::new();
            }
        }
        super::MainView::EditDocument { node_id } => {
            if let Some(corpus) = &app.project.selected_corpus {
                let job_title = "Creating document editor";
                let needs_refresh = force_refresh || app.current_editor.get().is_none();
                if needs_refresh && !app.jobs.has_active_job_with_title(job_title) {
                    app.current_editor = OnceLock::new();
                    let corpus_cache = app.project.corpus_cache.clone();
                    let location = corpus.location.clone();
                    app.jobs.add(
                        job_title,
                        move |_| {
                            let graph = corpus_cache.get(&location)?;
                            let document_editor =
                                DocumentEditor::create_from_graph(node_id, graph)?;

                            Ok(document_editor)
                        },
                        |document_editor, app| {
                            app.current_editor.get_or_init(|| Box::new(document_editor));
                        },
                    );
                }
            }
        }
        super::MainView::Demo => {
            app.current_editor = OnceLock::new();
        }
    }
}
