use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use cache::CorpusCache;
use graphannis::{update::GraphUpdate, AnnotationGraph};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{job_executor::JobExecutor, CorpusTree, Notifier, APP_ID};

mod cache;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Project {
    updates_pending: bool,
    pub(crate) selected_corpus: Option<String>,
    pub(crate) scheduled_for_deletion: Option<String>,
    pub(crate) corpus_locations: BTreeMap<String, PathBuf>,
    #[serde(skip)]
    corpus_cache: CorpusCache,
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
            corpus_cache: CorpusCache::default(),
            scheduled_for_deletion: None,
            corpus_locations: BTreeMap::new(),
            notifier,
        }
    }

    pub(crate) fn corpus_storage_dir(&self) -> Result<PathBuf> {
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
        if self.selected_corpus == selection {
            return;
        }

        self.selected_corpus = selection;
        self.schedule_corpus_tree_update(jobs);
    }

    pub(crate) fn new_empty_corpus(&mut self, name: &str) -> Result<()> {
        let id = Uuid::new_v4();
        let location = self.corpus_storage_dir()?.join(id.to_string());
        let mut graph = AnnotationGraph::with_default_graphstorages(false)?;
        graph.persist_to(&location)?;
        self.corpus_locations.insert(name.to_string(), location);
        Ok(())
    }

    pub(crate) fn add_changeset(&mut self, jobs: &JobExecutor, mut update: GraphUpdate) {
        if let Some(corpus_name) = self.selected_corpus.clone() {
            if let Some(corpus_location) = self.corpus_locations.get(&corpus_name).cloned() {
                self.updates_pending = true;
                let corpus_cache = self.corpus_cache.clone();
                jobs.add(
                    "Updating corpus",
                    move |job| {
                        job.update_message("Loading corpus if necessary");
                        if let Some(graph) = corpus_cache.get(&corpus_name, &corpus_location)? {
                            job.update_message("Applying updates");
                            let mut graph = graph.write();
                            graph.apply_update(&mut update, |msg| {
                                job.update_message(format!("Applying updates: {msg}"))
                            })?;
                        }

                        Ok(())
                    },
                    |_, app| {
                        app.project.updates_pending = false;
                        app.project.schedule_corpus_tree_update(&app.jobs);
                    },
                );
            }
        }
    }

    /// Rebuild the state that is not persisted but calculated
    pub(crate) fn load_after_init(&mut self, jobs: &JobExecutor) -> Result<()> {
        self.schedule_corpus_tree_update(jobs);
        Ok(())
    }

    fn schedule_corpus_tree_update(&mut self, jobs: &JobExecutor) {
        if let Some(corpus_name) = self.selected_corpus.clone() {
            if let Some(corpus_location) = self.corpus_locations.get(&corpus_name).cloned() {
                let notifier = self.notifier.clone();
                let corpus_cache = self.corpus_cache.clone();

                jobs.add(
                    "Updating corpus selection",
                    move |job| {
                        job.update_message("Loading corpus from disk");
                        if let Some(graph) = corpus_cache.get(&corpus_name, &corpus_location)? {
                            job.update_message("Updating corpus structure");
                            let corpus_tree = CorpusTree::create_from_graph(graph, notifier)?;
                            Ok(Some(corpus_tree))
                        } else {
                            Ok(None)
                        }
                    },
                    |result, app| {
                        let old_selection = app
                            .corpus_tree
                            .as_ref()
                            .and_then(|ct| ct.selected_corpus_node);
                        // Drop any old corpus tree in a background thread.
                        // The corpus tree can hold references to the annotation graph and occupy large amounts of memory, so dropping the memory in a background thread and don't block the UI
                        let old_corpus_tree = app.corpus_tree.take();
                        rayon::spawn(move || std::mem::drop(old_corpus_tree));

                        if let Some(mut corpus_tree) = result {
                            // Keep the selected corpus node
                            corpus_tree.select_corpus_node(old_selection);
                            app.corpus_tree = Some(corpus_tree);
                        }
                    },
                );
            }
        }
    }
}
