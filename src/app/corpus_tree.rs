use std::{collections::HashSet, fmt::Debug, sync::Arc};

use anyhow::Context;
use egui::{
    mutex::RwLock, Button, CollapsingHeader, Color32, Id, RichText, ScrollArea, TextEdit, Ui,
    Widget,
};
use egui_extras::{Column, TableRow};
use egui_notify::Toast;
use graphannis::{
    graph::{AnnoKey, Edge, NodeID, WriteableGraphStorage},
    model::{AnnotationComponent, AnnotationComponentType::PartOf},
    update::{
        GraphUpdate,
        UpdateEvent::{AddNodeLabel, DeleteNodeLabel},
    },
    AnnotationGraph,
};
use graphannis_core::{
    annostorage::ValueSearch,
    graph::{storage::adjacencylist::AdjacencyListStorage, ANNIS_NS, NODE_NAME_KEY, NODE_TYPE},
};

use super::job_executor::JobExecutor;
use super::Notifier;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
struct MetaEntry {
    current_namespace: String,
    current_name: String,
    current_value: String,
    original_namespace: String,
    original_name: String,
    original_value: String,
}

#[derive(Clone, PartialEq, Default, Debug)]
struct Data {
    parent_node_name: String,
    node_annos: Vec<MetaEntry>,
    changed_keys: HashSet<AnnoKey>,
    new_entry: MetaEntry,
}

pub(crate) struct CorpusTree {
    pub(crate) selected_corpus_node: Option<NodeID>,
    data: Data,
    gs: Box<dyn WriteableGraphStorage>,
    graph: Arc<RwLock<AnnotationGraph>>,
}

impl Debug for CorpusTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CorpusTree")
            .field("selected_corpus_node", &self.selected_corpus_node)
            .field("data", &self.data)
            .finish()
    }
}

impl CorpusTree {
    pub fn create_from_graph(graph: Arc<RwLock<AnnotationGraph>>) -> anyhow::Result<Self> {
        // Create our own graph storage with inverted edges
        let mut inverted_corpus_graph = AdjacencyListStorage::new();
        {
            let part_of_component = AnnotationComponent::new(PartOf, ANNIS_NS.into(), "".into());
            {
                let mut graph = graph.write();
                let all_partof_components = graph.get_all_components(Some(PartOf), None);
                graph.ensure_loaded_parallel(&all_partof_components)?;
            }
            let graph = graph.read();

            let partof = graph
                .get_graphstorage(&part_of_component)
                .context("Missing PartOf component")?;
            let corpus_nodes = graph.get_node_annos().exact_anno_search(
                Some(ANNIS_NS),
                NODE_TYPE,
                ValueSearch::Some("corpus"),
            );
            for source in corpus_nodes {
                let source = source?.node;
                for target in partof.get_outgoing_edges(source) {
                    let target = target?;
                    let edge = Edge { source, target };
                    inverted_corpus_graph.add_edge(edge.inverse())?;
                }
            }
            inverted_corpus_graph.calculate_statistics()?;
        }

        Ok(Self {
            selected_corpus_node: None,
            data: Data::default(),
            gs: Box::new(inverted_corpus_graph),
            graph,
        })
    }

    fn show_structure(&mut self, ui: &mut Ui, jobs: &JobExecutor, notifier: &Notifier) {
        let root_nodes: graphannis_core::errors::Result<Vec<_>> = self.gs.root_nodes().collect();
        let root_nodes = notifier.unwrap_or_default(root_nodes.context("Could not get root nodes"));
        ScrollArea::vertical().show(ui, |ui| {
            if root_nodes.len() > 1 {
                CollapsingHeader::new("<root>")
                    .default_open(true)
                    .show(ui, |ui| {
                        for root_node in root_nodes.iter() {
                            self.recursive_corpus_structure(ui, *root_node, 0, jobs, notifier)
                        }
                    });
            } else if let Some(root_node) = root_nodes.first() {
                self.recursive_corpus_structure(ui, *root_node, 0, jobs, notifier)
            }
        });
    }

    fn show_meta_editor(&mut self, ui: &mut Ui, jobs: &JobExecutor, notifier: &Notifier) {
        if self.selected_corpus_node.is_some() {
            let text_style_body = egui::TextStyle::Body.resolve(ui.style());

            // Calculate the heights needed for each line. Values with newline need more lines.

            egui_extras::TableBuilder::new(ui)
                .columns(Column::auto(), 3)
                .columns(Column::remainder(), 1)
                .auto_shrink(false)
                .header(text_style_body.size + 2.0, |mut header| {
                    header.col(|_ui| {});
                    header.col(|ui| {
                        ui.label(RichText::new("Namespace").underline());
                    });
                    header.col(|ui| {
                        ui.label(RichText::new("Name").underline());
                    });
                    header.col(|ui| {
                        ui.label(RichText::new("Value").underline());
                    });
                })
                .body(|body| {
                    body.rows(
                        text_style_body.size,
                        self.data.node_annos.len() + 1,
                        |mut row| {
                            if row.index() < self.data.node_annos.len() {
                                self.show_existing_metadata_entries(&mut row, jobs);
                            } else {
                                self.show_new_metadata_row(&mut row, jobs, notifier);
                            }
                        },
                    );
                });
        } else {
            ui.label("Select a corpus/document node to edit it.");
        }
    }

    fn show_existing_metadata_entries(&mut self, row: &mut TableRow<'_, '_>, jobs: &JobExecutor) {
        // Next rows are the actual ones
        let mut entry_idx = row.index();

        let anno_key_for_row = AnnoKey {
            ns: self.data.node_annos[entry_idx]
                .original_namespace
                .clone()
                .into(),
            name: self.data.node_annos[entry_idx].original_name.clone().into(),
        };

        row.col(|ui| {
            let delete_button = Button::new(RichText::new(egui_phosphor::regular::TRASH)).ui(ui);
            if delete_button.hovered() {
                delete_button.show_tooltip_text("Delete metadata entry");
            }
            if delete_button.clicked() {
                self.data.changed_keys.insert(anno_key_for_row.clone());
                self.data.node_annos.remove(entry_idx);
                if entry_idx >= self.data.node_annos.len() {
                    entry_idx = self.data.node_annos.len() - 1;
                }
                self.apply_pending_updates(jobs);
            };
        });

        let has_pending_changes = self.data.changed_keys.contains(&anno_key_for_row);
        let mut any_column_changed = false;
        let mut any_lost_focus = false;
        let entry = &mut self.data.node_annos[entry_idx];
        row.col(|ui| {
            let mut text_edit = TextEdit::singleline(&mut entry.current_namespace);
            if has_pending_changes {
                text_edit = text_edit.background_color(Color32::LIGHT_RED);
            }
            let text_edit = text_edit.ui(ui);

            if text_edit.changed() {
                any_column_changed = true;
            }
            if text_edit.lost_focus() {
                any_lost_focus = true;
            }
        });
        row.col(|ui| {
            let mut text_edit = TextEdit::singleline(&mut entry.current_name);
            if has_pending_changes {
                text_edit = text_edit.background_color(Color32::LIGHT_RED);
            }
            let text_edit = text_edit.ui(ui);

            if text_edit.changed() {
                any_column_changed = true;
            }
            if text_edit.lost_focus() {
                any_lost_focus = true;
            }
        });
        row.col(|ui| {
            let mut text_edit = TextEdit::singleline(&mut entry.current_value);
            if has_pending_changes {
                text_edit = text_edit.background_color(Color32::LIGHT_RED);
            }
            let text_edit = text_edit.ui(ui);

            if text_edit.changed() {
                any_column_changed = true;
            }
            if text_edit.lost_focus() {
                any_lost_focus = true;
            }
        });

        if any_column_changed {
            if entry.current_value == entry.original_value
                && entry.current_namespace == entry.original_namespace
                && entry.current_name == entry.original_name
            {
                self.data.changed_keys.remove(&anno_key_for_row);
            } else {
                self.data.changed_keys.insert(anno_key_for_row);
            }
        }
        if any_lost_focus && self.has_pending_updates() {
            self.apply_pending_updates(jobs);
        }
    }

    fn show_new_metadata_row(
        &mut self,
        row: &mut TableRow<'_, '_>,
        jobs: &JobExecutor,
        notifier: &Notifier,
    ) {
        row.col(|ui| {
            let add_button = Button::new(RichText::new(egui_phosphor::regular::PLUS_CIRCLE)).ui(ui);
            if add_button.hovered() {
                add_button.show_tooltip_text("Add new metadata entry");
            }

            if add_button.clicked() {
                self.add_new_entry(jobs, notifier);
            }
        });
        row.col(|ui| {
            TextEdit::singleline(&mut self.data.new_entry.current_namespace)
                .id(Id::from("new-metadata-entry-ns"))
                .ui(ui);
        });
        row.col(|ui| {
            TextEdit::singleline(&mut self.data.new_entry.current_name)
                .id(Id::from("new-metadata-entry-name"))
                .ui(ui);
        });
        row.col(|ui| {
            TextEdit::singleline(&mut self.data.new_entry.current_value)
                .id(Id::from("new-metadata-entry-value"))
                .ui(ui);
        });
    }

    fn add_new_entry(&mut self, jobs: &JobExecutor, notifier: &Notifier) {
        if self.data.new_entry.current_name.is_empty() {
            notifier.add_toast(Toast::error("Cannot add entry with empty name"));
        } else if self.data.node_annos.iter().any(|e| {
            e.current_namespace == self.data.new_entry.current_namespace
                && e.current_name == self.data.new_entry.current_name
        }) {
            notifier.add_toast(Toast::error(format!(
                "Entry with namespace \"{}\" and name \"{}\" already exists.",
                self.data.new_entry.current_namespace, self.data.new_entry.current_name
            )));
        } else {
            let new_entry = MetaEntry {
                current_namespace: self.data.new_entry.current_namespace.clone(),
                current_name: self.data.new_entry.current_name.clone(),
                current_value: self.data.new_entry.current_value.clone(),
                original_namespace: self.data.new_entry.current_namespace.clone(),
                original_name: self.data.new_entry.current_name.clone(),
                original_value: self.data.new_entry.current_value.clone(),
            };
            self.data.node_annos.push(new_entry);
            self.data.changed_keys.insert(AnnoKey {
                ns: self.data.new_entry.current_namespace.clone().into(),
                name: self.data.clone().new_entry.current_name.into(),
            });
            self.data.node_annos.sort();
            self.data.new_entry = MetaEntry::default();

            self.apply_pending_updates(jobs);
        }
    }
    pub(crate) fn has_pending_updates(&self) -> bool {
        !self.data.changed_keys.is_empty()
    }

    pub(crate) fn apply_pending_updates(&mut self, jobs: &JobExecutor) {
        if self.has_pending_updates() {
            // apply all changes as updates to our internal corpus graph
            let parent_node_name = self.data.parent_node_name.clone();
            let node_annos = self.data.node_annos.clone();
            let mut changed_keys = self.data.changed_keys.clone();
            jobs.add(
                "Applying pending metadata updates",
                move |_| {
                    let mut update = GraphUpdate::new();

                    for entry in node_annos.iter() {
                        let entry_key = AnnoKey {
                            ns: entry.original_namespace.clone().into(),
                            name: entry.original_name.clone().into(),
                        };
                        if changed_keys.contains(&entry_key) {
                            update.add_event(DeleteNodeLabel {
                                node_name: parent_node_name.clone(),
                                anno_ns: entry.original_namespace.clone(),
                                anno_name: entry.original_name.clone(),
                            })?;
                            update.add_event(AddNodeLabel {
                                node_name: parent_node_name.clone(),
                                anno_ns: entry.current_namespace.clone(),
                                anno_name: entry.current_name.clone(),
                                anno_value: entry.current_value.clone(),
                            })?;
                            changed_keys.remove(&entry_key);
                        }
                    }

                    // If there are any keys left that have not been used, these entries should be deleted
                    for entry_key in changed_keys.into_iter() {
                        update.add_event(DeleteNodeLabel {
                            node_name: parent_node_name.clone(),
                            anno_ns: entry_key.ns.to_string(),
                            anno_name: entry_key.name.to_string(),
                        })?;
                    }

                    Ok(update)
                },
                |update, app| {
                    app.project.add_changeset(&app.jobs, update);
                },
            );
            self.data.node_annos.sort();
            self.data.changed_keys.clear();
        }
    }

    pub(crate) fn show(&mut self, ui: &mut Ui, jobs: &JobExecutor, notifier: &Notifier) {
        ui.group(|ui| {
            ui.heading("Corpus editor");
            ui.columns_const(|[c1, c2]| {
                c1.push_id("corpus_structure", |ui| {
                    self.show_structure(ui, jobs, notifier)
                });
                c2.push_id("meta_editor", |ui| {
                    self.show_meta_editor(ui, jobs, notifier)
                });
            });
        });
    }

    pub(crate) fn select_corpus_node(&mut self, selection: Option<NodeID>, notifier: &Notifier) {
        self.selected_corpus_node = selection;
        if let Some(parent) = self.selected_corpus_node {
            self.data.node_annos.clear();
            self.data.changed_keys.clear();

            let graph = self.graph.read();
            let anno_keys = graph
                .get_node_annos()
                .get_all_keys_for_item(&parent, None, None);
            let anno_keys =
                notifier.unwrap_or_default(anno_keys.context("Could not get annotation keys"));
            for k in anno_keys {
                let anno_value = graph
                    .get_node_annos()
                    .get_value_for_item(&parent, &k)
                    .map(|v| v.unwrap_or_default().to_string());
                let anno_value = notifier
                    .unwrap_or_default(anno_value.context("Could not get annotation value"));
                self.data.node_annos.push(MetaEntry {
                    original_namespace: k.ns.to_string(),
                    original_name: k.name.to_string(),
                    original_value: anno_value.clone(),
                    current_namespace: k.ns.to_string(),
                    current_name: k.name.to_string(),
                    current_value: anno_value,
                });
            }
            let parent_node_name = graph
                .get_node_annos()
                .get_value_for_item(&parent, &NODE_NAME_KEY);
            let parent_node_name = notifier
                .unwrap_or_default(parent_node_name.context("Could not get parent node name"))
                .unwrap_or_default();
            self.data.parent_node_name = parent_node_name.to_string();
            self.data.node_annos.sort();
        }
    }

    fn recursive_corpus_structure(
        &mut self,
        ui: &mut Ui,
        parent: NodeID,
        level: usize,
        jobs: &JobExecutor,
        notifier: &Notifier,
    ) {
        let child_nodes: graphannis_core::errors::Result<Vec<NodeID>> =
            self.gs.get_outgoing_edges(parent).collect();
        let child_nodes =
            notifier.unwrap_or_default(child_nodes.context("Could not get child nodes"));
        let parent_node_name = {
            let graph = self.graph.read();
            match graph
                .get_node_annos()
                .get_value_for_item(&parent, &NODE_NAME_KEY)
            {
                Ok(o) => o.map(|o| o.to_string()),
                Err(e) => {
                    notifier.report_error(e.into());
                    None
                }
            }
        };

        if let Some(parent_node_name) = parent_node_name {
            if child_nodes.is_empty() {
                let is_selected = self.selected_corpus_node.is_some_and(|n| n == parent);

                let label = ui.selectable_label(is_selected, parent_node_name.clone());
                if label.clicked() {
                    self.apply_pending_updates(jobs);
                    label.request_focus();
                    if is_selected {
                        self.select_corpus_node(None, notifier);
                    } else {
                        self.select_corpus_node(Some(parent), notifier);
                    }
                }
            } else {
                CollapsingHeader::new(parent_node_name)
                    .default_open(level == 0)
                    .show(ui, |ui| {
                        for child_corpus in &child_nodes {
                            self.recursive_corpus_structure(
                                ui,
                                *child_corpus,
                                level + 1,
                                jobs,
                                notifier,
                            );
                        }
                    });
            }
        } else {
            notifier.add_toast(Toast::error("Node name not found"));
        }
    }
}
