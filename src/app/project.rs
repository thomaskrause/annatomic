use std::sync::Arc;

use anyhow::Context;
use graphannis::{update::GraphUpdate, CorpusStorage};
use serde::{Deserialize, Serialize};

use super::{job_executor::JobExecutor, CorpusTree, Notifier, APP_ID};

#[derive(Serialize, Deserialize)]
pub(crate) struct Project {
    updates_pending: bool,
    pub(crate) selected_corpus_name: Option<String>,
    pub(crate) scheduled_for_deletion: Option<String>,
    #[serde(skip)]
    pub(super) corpus_storage: Option<Arc<CorpusStorage>>,
    #[serde(skip)]
    notifier: Arc<Notifier>,
}

impl Project {
    pub(crate) fn new(notifier: Arc<Notifier>) -> Self {
        Self {
            updates_pending: false,
            selected_corpus_name: None,
            scheduled_for_deletion: None,
            corpus_storage: None,
            notifier,
        }
    }

    pub(crate) fn delete_corpus(&mut self, jobs: &JobExecutor, corpus_name: String) {
        if let Some(cs) = self.corpus_storage.as_ref().cloned() {
            let title = format!("Deleting corpus \"{corpus_name}\"");
            jobs.add(
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

    pub(crate) fn select_corpus(&mut self, jobs: &JobExecutor, selection: Option<String>) {
        self.selected_corpus_name = selection;
        self.schedule_corpus_tree_update(jobs);
    }

    pub(crate) fn add_changeset(&mut self, jobs: &JobExecutor, mut update: GraphUpdate) {
        if let Some(corpus_name) = self.selected_corpus_name.clone() {
            match self.ensure_corpus_storage_loaded() {
                Ok(cs) => {
                    self.updates_pending = true;
                    jobs.add(
                        "Updating corpus",
                        move |_job| {
                            cs.apply_update(&corpus_name, &mut update)?;
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
        self.ensure_corpus_storage_loaded()?;
        self.schedule_corpus_tree_update(jobs);
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

    fn schedule_corpus_tree_update(&mut self, jobs: &JobExecutor) {
        match self.ensure_corpus_storage_loaded() {
            Ok(cs) => {
                if let Some(corpus_name) = self.selected_corpus_name.clone() {
                    // Run a background job that creates the new corpus structure
                    let job_title = format!("Updating corpus structure for {}", &corpus_name);

                    let notifier = self.notifier.clone();
                    jobs.add(
                        &job_title,
                        move |_job| {
                            let corpus_tree =
                                CorpusTree::create_from_graphstorage(cs, &corpus_name, notifier)?;
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
            }
            Err(err) => {
                self.notifier.report_error(err);
            }
        }
    }
}
