use std::{collections::HashSet, sync::Arc};

use anyhow::{Context, Error};
use egui::{mutex::RwLock, Button, CollapsingHeader, RichText, ScrollArea, Ui};
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

use super::{job_executor::JobExecutor, Notifier, Project};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MetaEntry {
    current_namespace: String,
    current_name: String,
    current_value: String,
    original_namespace: String,
    original_name: String,
}

#[derive(Clone, PartialEq, Default)]
struct Data {
    parent_node_name: String,
    node_annos: Vec<MetaEntry>,
    changed_keys: HashSet<AnnoKey>,
}

impl TryFrom<&Data> for GraphUpdate {
    type Error = Error;

    fn try_from(value: &Data) -> Result<Self, Self::Error> {
        let mut update = GraphUpdate::new();

        for entry in value.node_annos.iter() {
            let entry_key = AnnoKey {
                ns: entry.original_namespace.clone().into(),
                name: entry.original_name.clone().into(),
            };
            if value.changed_keys.contains(&entry_key) {
                update.add_event(DeleteNodeLabel {
                    node_name: value.parent_node_name.clone(),
                    anno_ns: entry.original_namespace.clone(),
                    anno_name: entry.original_name.clone(),
                })?;
                update.add_event(AddNodeLabel {
                    node_name: value.parent_node_name.clone(),
                    anno_ns: entry.current_namespace.clone(),
                    anno_name: entry.current_name.clone(),
                    anno_value: entry.current_value.clone(),
                })?;
            }
        }

        Ok(update)
    }
}

pub(crate) struct CorpusTree {
    pub(crate) selected_corpus_node: Option<NodeID>,
    data: Data,
    gs: Box<dyn WriteableGraphStorage>,
    graph: Arc<RwLock<AnnotationGraph>>,
    notifier: Arc<Notifier>,
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

    fn show_structure(&mut self, ui: &mut Ui) {
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
                            self.recursive_corpus_structure(ui, *root_node, 0)
                        }
                    });
            } else if let Some(root_node) = root_nodes.first() {
                self.recursive_corpus_structure(ui, *root_node, 0)
            }
        });
    }

    fn show_meta_editor(&mut self, ui: &mut Ui, project: &mut Project, jobs: &JobExecutor) {
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

            if ui
                .add_enabled(
                    !self.data.changed_keys.is_empty(),
                    Button::new("Apply document updates"),
                )
                .clicked()
            {
                // apply all changes as updates to our internal corpus graph
                match GraphUpdate::try_from(&self.data) {
                    Ok(update) => project.add_changeset(jobs, update),
                    Err(err) => self.notifier.report_error(err),
                }
                self.data.node_annos.sort();
                // TODO: record these in the project manager and update the actual corpus
            }
        } else {
            ui.label("Select a corpus/document node to edit it.");
        }
    }

    pub(crate) fn show(&mut self, ui: &mut Ui, project: &mut Project, jobs: &JobExecutor) {
        ui.group(|ui| {
            ui.heading("Corpus editor");
            ui.columns_const(|[c1, c2]| {
                c1.push_id("corpus_structure", |ui| self.show_structure(ui));
                c2.push_id("meta_editor", |ui| self.show_meta_editor(ui, project, jobs));
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

    fn recursive_corpus_structure(&mut self, ui: &mut Ui, parent: NodeID, level: usize) {
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
                if ui
                    .selectable_label(is_selected, parent_node_name.as_str())
                    .clicked()
                {
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
                            self.recursive_corpus_structure(ui, *child_corpus, level + 1);
                        }
                    });
            }
        } else {
            self.notifier.add_toast(Toast::error("Node name not found"));
        }
    }
}
