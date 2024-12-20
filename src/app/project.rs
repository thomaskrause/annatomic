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
    pub(crate) graph: Option<Arc<RwLock<AnnotationGraph>>>,
}

impl SelectedCorpus {
    fn ensure_graph_loaded(&mut self) -> anyhow::Result<Arc<RwLock<AnnotationGraph>>> {
        if let Some(graph) = &self.graph {
            Ok(graph.clone())
        } else {
            // Load the corpus from the location
            let mut graph = AnnotationGraph::new(false)?;
            graph.load_from(&self.location, true)?;
            let graph = RwLock::new(graph);
            let graph = Arc::new(graph);
            self.graph = Some(graph.clone());
            Ok(graph)
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Project {
    updates_pending: bool,
    pub(crate) selected_corpus: Option<SelectedCorpus>,
    pub(crate) scheduled_for_deletion: Option<String>,
    pub(crate) corpus_locations: BTreeMap<String, PathBuf>,
    #[serde(skip)]
    notifier: Arc<Notifier>,
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
            match selected_corpus.ensure_graph_loaded() {
                Ok(graph) => {
                    self.updates_pending = true;
                    jobs.add(
                        "Updating corpus",
                        move |job| {
                            let mut graph = graph.write().map_err(|e| anyhow!("{e}"))?;
                            graph.apply_update(&mut update, |msg| job.update_message(msg))?;
                            Ok(())
                        },
                        |_result, app| {
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
        if let Some(selected_corpus) = &mut self.selected_corpus {
            selected_corpus.ensure_graph_loaded()?;
        }
        self.schedule_corpus_tree_update(jobs);
        Ok(())
    }

    fn schedule_corpus_tree_update(&mut self, jobs: &JobExecutor) {
        if let Some(selected_corpus) = &mut self.selected_corpus {
            match selected_corpus.ensure_graph_loaded() {
                Ok(graph) => {
                    // Run a background job that creates the new corpus structure
                    let job_title =
                        format!("Updating corpus structure for {}", &selected_corpus.name);

                    let notifier = self.notifier.clone();
                    jobs.add(
                        &job_title,
                        move |_job| {
                            let corpus_tree =
                                CorpusTree::create_from_graph(graph.clone(), notifier)?;
                            Ok(corpus_tree)
                        },
                        |mut result, app| {
                            // Keep the selected corpus node
                            let old_selection = app
                                .corpus_tree
                                .as_ref()
                                .and_then(|ct| ct.selected_corpus_node);
                            result.select_corpus_node(old_selection);
                            app.corpus_tree = Some(result);
                        },
                    );
                }
                Err(err) => {
                    self.notifier.report_error(err);
                }
            }
        }
    }
}
