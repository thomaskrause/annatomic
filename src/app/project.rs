use std::sync::Arc;

use anyhow::Context;
use graphannis::CorpusStorage;
use serde::{Deserialize, Serialize};

use super::{job_executor::JobExecutor, CorpusTree, Notifier, APP_ID};

#[derive(Serialize, Deserialize)]
pub(crate) struct Project {
    updates_pending: bool,
    pub(crate) name: Option<String>,
    pub(crate) scheduled_for_deletion: Option<String>,
    #[serde(skip)]
    pub(super) corpus_storage: Option<Arc<CorpusStorage>>,
    #[serde(skip)]
    pub(crate) corpus_tree: Option<CorpusTree>,
    #[serde(skip)]
    notifier: Arc<Notifier>,
    #[serde(skip)]
    jobs: Arc<JobExecutor>,
}

impl Project {
    pub(crate) fn new(notifier: Arc<Notifier>, jobs: Arc<JobExecutor>) -> Self {
        Self {
            updates_pending: false,
            name: None,
            scheduled_for_deletion: None,
            corpus_storage: None,
            corpus_tree: None,
            notifier,
            jobs,
        }
    }

    pub(crate) fn delete_corpus(&mut self, corpus_name: String) {
        if let Some(cs) = self.corpus_storage.as_ref().cloned() {
            let title = format!("Deleting corpus \"{corpus_name}\"");
            self.jobs.add(
                &title,
                move |_job| {
                    cs.delete(&corpus_name)?;
                    Ok(())
                },
                |_result, _app| {},
            );
        }
        self.scheduled_for_deletion = None;
    }

    pub(crate) fn select_corpus(&mut self, selection: Option<String>) {
        self.name = selection;
        self.schedule_corpus_tree_update();
    }

    /// Rebuild the state that is not persisted but calculated
    pub(crate) fn load_after_init(&mut self) -> anyhow::Result<()> {
        self.ensure_corpus_storage_loaded()?;
        self.schedule_corpus_tree_update();
        Ok(())
    }

    fn ensure_corpus_storage_loaded(&mut self) -> anyhow::Result<Arc<CorpusStorage>> {
        if let Some(cs) = &self.corpus_storage {
            Ok(cs.clone())
        } else {
            let parent_path =
                eframe::storage_dir(APP_ID).context("Unable to get local file storage path")?;
            // Attempt to create a corpus storage and remember it
            let cs = CorpusStorage::with_auto_cache_size(&parent_path.join("db"), true)?;
            let cs = Arc::new(cs);
            self.corpus_storage = Some(cs.clone());
            Ok(cs)
        }
    }

    fn schedule_corpus_tree_update(&mut self) {
        match self.ensure_corpus_storage_loaded() {
            Ok(cs) => {
                if let Some(corpus_name) = self.name.clone() {
                    // Run a background job that creates the new corpus structure

                    let job_title = format!("Updating corpus structure for {}", &corpus_name);
                    let notifier = self.notifier.clone();
                    self.jobs.add(
                        &job_title,
                        move |_job| {
                            let corpus_tree =
                                CorpusTree::create_from_graphstorage(cs, &corpus_name, notifier)?;
                            Ok(corpus_tree)
                        },
                        |result, app| {
                            app.project.corpus_tree = Some(result);
                        },
                    );
                } else {
                    self.corpus_tree = None;
                }
            }
            Err(err) => {
                self.notifier.report_error(err);
            }
        }
    }
}
