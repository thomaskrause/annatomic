use std::{collections::HashSet, fmt::Debug, sync::Arc};

use anyhow::Context;
use egui::{mutex::RwLock, CollapsingHeader, RichText, ScrollArea, Ui};
use egui_extras::Column;
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MetaEntry {
    current_namespace: String,
    current_name: String,
    current_value: String,
    original_namespace: String,
    original_name: String,
}

#[derive(Clone, PartialEq, Default, Debug)]
struct Data {
    parent_node_name: String,
    node_annos: Vec<MetaEntry>,
    changed_keys: HashSet<AnnoKey>,
}

pub(crate) struct CorpusTree {
    pub(crate) selected_corpus_node: Option<NodeID>,
    data: Data,
    gs: Box<dyn WriteableGraphStorage>,
    graph: Arc<RwLock<AnnotationGraph>>,
    notifier: Arc<Notifier>,
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
    pub fn create_from_graph(
        graph: Arc<RwLock<AnnotationGraph>>,
        notifier: Arc<Notifier>,
    ) -> anyhow::Result<Self> {
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
            notifier,
        })
    }

    fn show_structure(&mut self, ui: &mut Ui, jobs: &JobExecutor) {
        let root_nodes: graphannis_core::errors::Result<Vec<_>> = self.gs.root_nodes().collect();
        let root_nodes = self
            .notifier
            .unwrap_or_default(root_nodes.context("Could not get root nodes"));
        ScrollArea::vertical().show(ui, |ui| {
            if root_nodes.len() > 1 {
                CollapsingHeader::new("<root>")
                    .default_open(true)
                    .show(ui, |ui| {
                        for root_node in root_nodes.iter() {
                            self.recursive_corpus_structure(ui, *root_node, 0, jobs)
                        }
                    });
            } else if let Some(root_node) = root_nodes.first() {
                self.recursive_corpus_structure(ui, *root_node, 0, jobs)
            }
        });
    }

    fn show_meta_editor(&mut self, ui: &mut Ui) {
        if self.selected_corpus_node.is_some() {
            let text_style_body = egui::TextStyle::Body.resolve(ui.style());
            egui_extras::TableBuilder::new(ui)
                .columns(Column::auto(), 2)
                .columns(Column::remainder(), 1)
                .header(text_style_body.size + 2.0, |mut header| {
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
                .body(|mut body| {
                    for entry in self.data.node_annos.iter_mut() {
                        body.row(text_style_body.size, |mut row| {
                            row.col(|ui| {
                                if ui
                                    .text_edit_singleline(&mut entry.current_namespace)
                                    .changed()
                                {
                                    self.data.changed_keys.insert(AnnoKey {
                                        ns: entry.original_namespace.clone().into(),
                                        name: entry.original_name.clone().into(),
                                    });
                                }
                            });
                            row.col(|ui| {
                                if ui.text_edit_singleline(&mut entry.current_name).changed() {
                                    self.data.changed_keys.insert(AnnoKey {
                                        ns: entry.original_namespace.clone().into(),
                                        name: entry.original_name.clone().into(),
                                    });
                                }
                            });
                            row.col(|ui| {
                                if ui.text_edit_singleline(&mut entry.current_value).changed() {
                                    self.data.changed_keys.insert(AnnoKey {
                                        ns: entry.original_namespace.clone().into(),
                                        name: entry.original_name.clone().into(),
                                    });
                                }
                            });
                        });
                    }
                });
        } else {
            ui.label("Select a corpus/document node to edit it.");
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
            let changed_keys = self.data.changed_keys.clone();
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
                        }
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

    pub(crate) fn show(&mut self, ui: &mut Ui, jobs: &JobExecutor) {
        ui.group(|ui| {
            ui.heading("Corpus editor");
            ui.columns_const(|[c1, c2]| {
                c1.push_id("corpus_structure", |ui| self.show_structure(ui, jobs));
                c2.push_id("meta_editor", |ui| self.show_meta_editor(ui));
            });
        });
    }

    pub(crate) fn select_corpus_node(&mut self, selection: Option<NodeID>) {
        self.selected_corpus_node = selection;
        if let Some(parent) = self.selected_corpus_node {
            self.data.node_annos.clear();
            self.data.changed_keys.clear();

            let graph = self.graph.read();
            let anno_keys = graph
                .get_node_annos()
                .get_all_keys_for_item(&parent, None, None);
            let anno_keys = self
                .notifier
                .unwrap_or_default(anno_keys.context("Could not get annotation keys"));
            for k in anno_keys {
                let anno_value = graph
                    .get_node_annos()
                    .get_value_for_item(&parent, &k)
                    .map(|v| v.unwrap_or_default().to_string());
                let anno_value = self
                    .notifier
                    .unwrap_or_default(anno_value.context("Could not get annotation value"));
                self.data.node_annos.push(MetaEntry {
                    original_namespace: k.ns.to_string(),
                    original_name: k.name.to_string(),
                    current_namespace: k.ns.to_string(),
                    current_name: k.name.to_string(),
                    current_value: anno_value,
                });
            }
            let parent_node_name = graph
                .get_node_annos()
                .get_value_for_item(&parent, &NODE_NAME_KEY);
            let parent_node_name = self
                .notifier
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
    ) {
        let child_nodes: graphannis_core::errors::Result<Vec<NodeID>> =
            self.gs.get_outgoing_edges(parent).collect();
        let child_nodes = self
            .notifier
            .unwrap_or_default(child_nodes.context("Could not get child nodes"));
        let parent_node_name = {
            let graph = self.graph.read();
            match graph
                .get_node_annos()
                .get_value_for_item(&parent, &NODE_NAME_KEY)
            {
                Ok(o) => o.map(|o| o.to_string()),
                Err(e) => {
                    self.notifier.report_error(e.into());
                    None
                }
            }
        };

        if let Some(parent_node_name) = parent_node_name {
            if child_nodes.is_empty() {
                let is_selected = self.selected_corpus_node.is_some_and(|n| n == parent);
                let is_changed = is_selected && self.has_pending_updates();
                let name = if is_changed {
                    format!("*{parent_node_name}")
                } else {
                    parent_node_name.clone()
                };
                if ui.selectable_label(is_selected, name).clicked() {
                    self.apply_pending_updates(jobs);
                    if is_selected {
                        self.select_corpus_node(None);
                    } else {
                        self.select_corpus_node(Some(parent));
                    }
                }
            } else {
                CollapsingHeader::new(parent_node_name)
                    .default_open(level == 0)
                    .show(ui, |ui| {
                        for child_corpus in &child_nodes {
                            self.recursive_corpus_structure(ui, *child_corpus, level + 1, jobs);
                        }
                    });
            }
        } else {
            self.notifier.add_toast(Toast::error("Node name not found"));
        }
    }
}
