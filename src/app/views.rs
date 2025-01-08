use super::{AnnatomicApp, CorpusTree, DocumentEditor};
use std::sync::OnceLock;
pub(crate) mod demo;
pub(crate) mod edit;
pub(crate) mod start;

#[derive(Default)]
pub(crate) struct LoadedViewComponents {
    pub(crate) corpus_tree: OnceLock<CorpusTree>,
    pub(crate) document_editor: OnceLock<DocumentEditor>,
}

pub(crate) fn load_components_for_view(app: &mut AnnatomicApp) {
    match app.main_view {
        super::MainView::Start => {
            app.view_components.document_editor = OnceLock::new();
            if let Some(corpus) = &app.project.selected_corpus {
                let job_title = "Creating corpus tree";
                if app.view_components.corpus_tree.get().is_none()
                    && !app.jobs.has_active_job_with_title(&job_title)
                {
                    let corpus_cache = app.project.corpus_cache.clone();
                    let jobs = app.jobs.clone();
                    let notifier = app.notifier.clone();
                    let location = corpus.location.clone();
                    app.jobs.add(
                        &job_title,
                        move |_| {
                            let graph = corpus_cache.get(&location)?;
                            let corpus_tree = CorpusTree::create_from_graph(graph, jobs, notifier)?;
                            Ok(corpus_tree)
                        },
                        |corpus_tree, app| {
                            app.view_components.corpus_tree.get_or_init(|| corpus_tree);
                        },
                    );
                }
            } else {
                app.view_components.corpus_tree = OnceLock::new();
            }
        }
        super::MainView::EditDocument { .. } => {
            app.view_components.corpus_tree = OnceLock::new();
            app.view_components.document_editor = OnceLock::new();
        }
        super::MainView::Demo => {
            app.view_components.corpus_tree = OnceLock::new();
            app.view_components.document_editor = OnceLock::new();
        }
    }
}
