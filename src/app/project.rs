use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use anyhow::{anyhow, Context};
use graphannis::{update::GraphUpdate, AnnotationGraph};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{job_executor::JobExecutor, CorpusTree, Notifier, APP_ID};

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct SelectedCorpus {
    pub(crate) name: String,
    pub(crate) location: PathBuf,
    #[serde(skip)]
    graph: Option<Arc<RwLock<AnnotationGraph>>>,
}

impl PartialEq for SelectedCorpus {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.location == other.location
    }
}

impl SelectedCorpus {
    /// Gets the graph for this selected corpus.
    ///
    /// The first call might take some time because the graph needs to be loaded
    /// from disk. Later calls are cached and are faster, but you should make
    /// sure to not call this function in a blocking environment.
    fn graph(&mut self) -> anyhow::Result<Arc<RwLock<AnnotationGraph>>> {
        if let Some(graph) = &self.graph {
            Ok(graph.clone())
        } else {
            // Load the corpus from the location
            let mut graph = AnnotationGraph::new(false)?;
            graph.load_from(&self.location, false)?;
            let graph = RwLock::new(graph);
            let graph = Arc::new(graph);
            self.graph = Some(graph.clone());
            Ok(graph)
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Project {
    updates_pending: bool,
    pub(crate) selected_corpus: Option<SelectedCorpus>,
    pub(crate) scheduled_for_deletion: Option<String>,
    pub(crate) corpus_locations: BTreeMap<String, PathBuf>,
    #[serde(skip)]
    notifier: Arc<Notifier>,
}

impl PartialEq for Project {
    fn eq(&self, other: &Self) -> bool {
        self.updates_pending == other.updates_pending
            && self.selected_corpus == other.selected_corpus
            && self.scheduled_for_deletion == other.scheduled_for_deletion
            && self.corpus_locations == other.corpus_locations
    }
}

impl Project {
    pub(crate) fn new(notifier: Arc<Notifier>) -> Self {
        Self {
            updates_pending: false,
            selected_corpus: None,
            scheduled_for_deletion: None,
            corpus_locations: BTreeMap::new(),
            notifier,
        }
    }

    pub(crate) fn corpus_storage_dir(&self) -> anyhow::Result<PathBuf> {
        let result = eframe::storage_dir(APP_ID)
            .context("Unable to get local file storage path")
            .map(|p| p.join("corpora"))?;
        Ok(result)
    }

    pub(crate) fn delete_corpus(&mut self, jobs: &JobExecutor, corpus_name: String) {
        if let Some(location) = self.corpus_locations.remove(&corpus_name) {
            let title = format!(
                "Deleting corpus \"{corpus_name}\" from {}",
                location.to_string_lossy()
            );
            jobs.add(
                &title,
                move |_job| {
                    std::fs::remove_dir_all(location)?;
                    Ok(())
                },
                |_result, _app| {},
            );
        }
        self.scheduled_for_deletion = None;
    }

    pub(crate) fn select_corpus(&mut self, jobs: &JobExecutor, selection: Option<String>) {
        // Do nothing if the corpus is already selected
        if self
            .selected_corpus
            .as_ref()
            .and_then(|s| Some(s.name.clone()))
            == selection
        {
            return;
        }

        self.selected_corpus = if let Some(name) = selection {
            self.corpus_locations
                .get(&name)
                .map(|location| SelectedCorpus {
                    graph: None,
                    name,
                    location: location.clone(),
                })
        } else {
            None
        };
        self.schedule_corpus_tree_update(jobs);
    }

    pub(crate) fn new_empty_corpus(&mut self, name: &str) -> anyhow::Result<()> {
        let id = Uuid::new_v4();
        let location = self.corpus_storage_dir()?.join(id.to_string());
        let mut graph = AnnotationGraph::with_default_graphstorages(false)?;
        graph.persist_to(&location)?;
        self.corpus_locations.insert(name.to_string(), location);
        Ok(())
    }

    pub(crate) fn add_changeset(&mut self, jobs: &JobExecutor, mut update: GraphUpdate) {
        if let Some(selected_corpus) = &mut self.selected_corpus {
            match selected_corpus.graph() {
                Ok(graph) => {
                    self.updates_pending = true;
                    jobs.add(
                        "Updating corpus",
                        move |job| {
                            let mut graph = graph.write().map_err(|e| anyhow!("{e}"))?;
                            graph.apply_update(&mut update, |msg| job.update_message(msg))?;
                            Ok(())
                        },
                        |_, app| {
                            app.project.updates_pending = false;
                            app.project.schedule_corpus_tree_update(&app.jobs);
                        },
                    );
                }
                Err(err) => self.notifier.report_error(err),
            }
        }
    }

    /// Rebuild the state that is not persisted but calculated
    pub(crate) fn load_after_init(&mut self, jobs: &JobExecutor) -> anyhow::Result<()> {
        self.schedule_corpus_tree_update(jobs);
        Ok(())
    }

    fn schedule_corpus_tree_update(&mut self, jobs: &JobExecutor) {
        let selected_corpus = self.selected_corpus.clone();

        let notifier = self.notifier.clone();
        jobs.add(
            "Updating corpus selection",
            |job| {
                if let Some(mut selected_corpus) = selected_corpus {
                    job.update_message("Loading corpus from disk");
                    let graph = selected_corpus.graph()?;
                    job.update_message("Updating corpus structure");
                    let corpus_tree = CorpusTree::create_from_graph(graph.clone(), notifier)?;
                    Ok(Some(corpus_tree))
                } else {
                    Ok(None)
                }
            },
            |result, app| {
                // Drop any old corpus tree in a background thread.
                // The corpus tree can hold references to the annotation graph and occupy large amounts of memory, so dropping the memory in a background thread and don't block the UI
                let old_corpus_tree = app.corpus_tree.take();
                rayon::spawn(move || std::mem::drop(old_corpus_tree));

                if let Some(mut corpus_tree) = result {
                    // Keep the selected corpus node
                    let old_selection = app
                        .corpus_tree
                        .as_ref()
                        .and_then(|ct| ct.selected_corpus_node);
                    corpus_tree.select_corpus_node(old_selection);
                    app.corpus_tree = Some(corpus_tree);
                }
            },
        );
    }
}
