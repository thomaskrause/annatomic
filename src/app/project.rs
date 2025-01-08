use std::{
    collections::BTreeMap,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use anyhow::{Context, Ok, Result};
use cache::CorpusCache;

use egui::util::undoer::{self, Undoer};
use egui_notify::Toast;
use graphannis::{
    update::{GraphUpdate, UpdateEvent},
    AnnotationGraph,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{job_executor::JobExecutor, load_components_for_view};
use super::{Notifier, APP_ID};

#[cfg(test)]
use egui::mutex::RwLock;
#[cfg(test)]
use std::sync::Arc;

mod cache;
#[cfg(test)]
mod tests;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct Corpus {
    pub(crate) name: String,
    pub(crate) location: PathBuf,
    diff_to_last_save: Vec<UpdateEvent>,
}

impl Corpus {
    pub(crate) fn new<S, P>(name: S, location: P) -> Self
    where
        S: Into<String>,
        P: Into<PathBuf>,
    {
        Self {
            name: name.into(),
            location: location.into(),
            diff_to_last_save: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Project {
    updates_pending: bool,
    pub(crate) selected_corpus: Option<Corpus>,
    pub(crate) scheduled_for_deletion: Option<String>,
    pub(crate) corpus_locations: BTreeMap<String, PathBuf>,
    #[serde(skip)]
    pub(super) corpus_cache: CorpusCache,
    #[serde(skip)]
    notifier: Notifier,
    #[serde(skip)]
    jobs: JobExecutor,
    #[serde(skip)]
    undoer: Undoer<Corpus>,
}

fn default_undoer() -> Undoer<Corpus> {
    let undo_settings = undoer::Settings {
        max_undos: 10,
        ..Default::default()
    };
    Undoer::with_settings(undo_settings)
}

impl Project {
    pub(crate) fn new(notifier: Notifier, jobs: JobExecutor) -> Self {
        Self {
            updates_pending: false,
            selected_corpus: None,
            corpus_cache: CorpusCache::default(),
            scheduled_for_deletion: None,
            corpus_locations: BTreeMap::new(),
            notifier,
            jobs,
            undoer: default_undoer(),
        }
    }

    pub(crate) fn corpus_storage_dir(&self) -> Result<PathBuf> {
        let result = eframe::storage_dir(APP_ID)
            .context("Unable to get local file storage path")
            .map(|p| p.join("corpora"))?;
        Ok(result)
    }

    pub(crate) fn delete_corpus(&mut self, corpus_name: String) {
        self.scheduled_for_deletion = None;

        // Delete the folder where the corpus is stored
        if let Some(location) = self.corpus_locations.remove(&corpus_name) {
            let title = format!(
                "Deleting corpus \"{corpus_name}\" from {}",
                location.to_string_lossy()
            );
            self.jobs.add(
                &title,
                move |_job| {
                    std::fs::remove_dir_all(location)?;
                    Ok(())
                },
                |_result, app| {
                    app.project.select_corpus(None);
                    load_components_for_view(app, true);
                },
            );
        }
    }

    pub(super) fn select_corpus(&mut self, selection: Option<String>) {
        // Do nothing if the corpus is already selected
        if let Some(selected_corpus) = &self.selected_corpus {
            if Some(&selected_corpus.name) == selection.as_ref() {
                return;
            }
        }

        self.selected_corpus = None;
        if let Some(name) = selection {
            if let Some(location) = self.corpus_locations.get(&name) {
                let new_selection = Corpus::new(name, location);
                self.undoer = default_undoer();
                self.undoer.add_undo(&new_selection);
                self.selected_corpus = Some(new_selection);
            } else {
                self.notifier
                    .add_toast(Toast::error(format!("Missing location for corpus {name}")));
            }
        }
    }

    pub(crate) fn new_empty_corpus(&mut self, name: &str) -> Result<()> {
        let id = Uuid::new_v4();
        let location = self.corpus_storage_dir()?.join(id.to_string());
        let mut graph = AnnotationGraph::with_default_graphstorages(false)?;
        graph.persist_to(&location)?;
        self.corpus_locations.insert(name.to_string(), location);
        Ok(())
    }

    pub(crate) fn add_changeset(&mut self, mut update: GraphUpdate) {
        if let Some(selected_corpus) = self.selected_corpus.clone() {
            self.updates_pending = true;
            let corpus_cache = self.corpus_cache.clone();
            self.jobs.add(
                "Updating corpus",
                move |job| {
                    job.update_message("Storing update events");
                    let mut added_events = Vec::with_capacity(update.len()?);
                    for event in update.iter()? {
                        let event = event?;
                        added_events.push(event.1);
                    }
                    job.update_message("Loading corpus if necessary");
                    let graph = corpus_cache.get(&selected_corpus.location)?;
                    job.update_message("Applying updates");
                    let mut graph = graph.write();
                    graph.apply_update_keep_statistics(&mut update, |msg| {
                        job.update_message(format!("Applying updates: {msg}"))
                    })?;

                    Ok(added_events)
                },
                |added_events, app| {
                    if let Some(selected_corpus) = &mut app.project.selected_corpus {
                        selected_corpus.diff_to_last_save.extend(added_events);
                        app.project.undoer.add_undo(selected_corpus);
                    }
                    app.project.updates_pending = false;
                },
            );
        }
    }

    pub(crate) fn persist_changes_on_exit(&mut self) -> Result<()> {
        if let Some(selected_corpus) = self.selected_corpus.clone() {
            self.updates_pending = true;
            let corpus_cache = self.corpus_cache.clone();
            let graph = corpus_cache.get(&selected_corpus.location)?;
            let mut graph = graph.write();
            graph.persist_to(&selected_corpus.location)?;
            self.undoer = default_undoer();
        }
        Ok(())
    }

    pub(crate) fn export_to_graphml(&self, location: &Path) {
        if let Some(selected_corpus) = self.selected_corpus.clone() {
            let corpus_cache = self.corpus_cache.clone();
            let job_title = format!("Exporting {}", location.to_string_lossy());
            let location = location.to_path_buf();
            self.jobs.add(
                &job_title,
                move |job| {
                    let graph = corpus_cache.get(&selected_corpus.location)?;
                    let outfile = File::create(location)?;
                    let buffered_writer = BufWriter::new(outfile);
                    let graph = graph.read();
                    graphannis_core::graph::serialization::graphml::export_stable_order(
                        &graph,
                        None,
                        buffered_writer,
                        |msg| {
                            job.update_message(msg);
                        },
                    )?;

                    Ok(())
                },
                |_, _| {},
            );
        }
    }

    pub(crate) fn has_undo(&self) -> bool {
        self.selected_corpus
            .as_ref()
            .is_some_and(|c| self.undoer.has_undo(c))
    }

    pub(crate) fn undo(&mut self) {
        if let Some(selected_corpus) = &mut self.selected_corpus {
            if let Some(new_state) = self.undoer.undo(selected_corpus).cloned() {
                self.selected_corpus = Some(new_state.clone());
                let corpus_cache = self.corpus_cache.clone();
                // Reload the corpus from disk and apply the outstanding changes
                self.jobs.add(
                    "Undoing changes",
                    move |j| {
                        j.update_message("Loading old corpus state from disk");
                        let lock = corpus_cache.load_from_disk(&new_state.location)?;
                        {
                            let mut graph = lock.write();
                            j.update_message("Applying updates");
                            let mut updates = GraphUpdate::new();
                            for event in new_state.diff_to_last_save.iter() {
                                updates.add_event(event.clone())?;
                            }
                            graph.apply_update_keep_statistics(&mut updates, |msg| {
                                j.update_message(format!("Applying updates: {}", msg));
                            })?;
                        }
                        Ok(lock)
                    },
                    |_, app| {
                        load_components_for_view(app, true);
                    },
                );
            }
        }
    }

    pub(crate) fn has_redo(&self) -> bool {
        self.selected_corpus
            .as_ref()
            .is_some_and(|c| self.undoer.has_redo(c))
    }

    pub(crate) fn redo(&mut self) {
        if let Some(selected_corpus) = &mut self.selected_corpus {
            if let Some(new_state) = self.undoer.redo(selected_corpus).cloned() {
                self.selected_corpus = Some(new_state.clone());
                let corpus_cache = self.corpus_cache.clone();
                // Reload the corpus from disk and apply the outstanding changes
                self.jobs.add(
                    "Redoing changes",
                    move |j| {
                        j.update_message("Loading old corpus state from disk");
                        let lock = corpus_cache.load_from_disk(&new_state.location)?;
                        {
                            let mut graph = lock.write();
                            j.update_message("Applying updates");
                            let mut updates = GraphUpdate::new();
                            for event in new_state.diff_to_last_save.iter() {
                                updates.add_event(event.clone())?;
                            }
                            graph.apply_update_keep_statistics(&mut updates, |msg| {
                                j.update_message(format!("Applying updates: {}", msg));
                            })?;
                        }
                        Ok(lock)
                    },
                    |_, app| {
                        load_components_for_view(app, true);
                    },
                );
            }
        }
    }

    /// Rebuild the state that is not persisted but calculated
    pub(crate) fn load_after_init(&mut self, notifier: Notifier, jobs: JobExecutor) -> Result<()> {
        self.notifier = notifier;
        self.jobs = jobs;
        if let Some(selection) = &mut self.selected_corpus {
            selection.diff_to_last_save.clear();
            self.undoer.add_undo(selection);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn get_selected_graph(&self) -> Result<Option<Arc<RwLock<AnnotationGraph>>>> {
        if let Some(corpus) = &self.selected_corpus {
            let graph = self.corpus_cache.get(&corpus.location)?;
            Ok(Some(graph))
        } else {
            Ok(None)
        }
    }
}
