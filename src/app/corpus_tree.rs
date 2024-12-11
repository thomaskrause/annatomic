use std::sync::Arc;

use anyhow::Context;
use egui::{CollapsingHeader, ScrollArea, Ui};
use egui_extras::Column;
use egui_notify::Toast;
use graphannis::{
    graph::{Edge, NodeID, WriteableGraphStorage},
    model::{AnnotationComponent, AnnotationComponentType::PartOf},
    AnnotationGraph, CorpusStorage,
};
use graphannis_core::graph::{
    storage::adjacencylist::AdjacencyListStorage, ANNIS_NS, NODE_NAME_KEY,
};

use super::Notifier;

pub(crate) struct CorpusTree {
    pub(crate) selected_corpus_node: Option<NodeID>,
    gs: Box<dyn WriteableGraphStorage>,
    corpus_graph: AnnotationGraph,
    notifier: Arc<Notifier>,
}

impl CorpusTree {
    pub fn create_from_graphstorage(
        cs: Arc<CorpusStorage>,
        corpus_name: &str,
        notifier: Arc<Notifier>,
    ) -> anyhow::Result<Self> {
        let mut corpus_graph = cs.corpus_graph(corpus_name)?;
        corpus_graph.ensure_loaded_all()?;

        // Create our own graph storage with inverted edges
        let mut inverted_corpus_graph = AdjacencyListStorage::new();

        let partof = corpus_graph
            .get_or_create_writable(&AnnotationComponent::new(
                PartOf,
                ANNIS_NS.into(),
                "".into(),
            ))
            .context("Missing PartOf component")?;
        for source in partof.source_nodes() {
            let source = source?;
            for target in partof.get_outgoing_edges(source) {
                let target = target?;
                let edge = Edge { source, target };
                inverted_corpus_graph.add_edge(edge.inverse())?;
            }
        }
        inverted_corpus_graph.calculate_statistics()?;

        Ok(Self {
            selected_corpus_node: None,
            gs: Box::new(inverted_corpus_graph),
            corpus_graph,
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

    fn show_meta_editor(&mut self, ui: &mut Ui) {
        if let Some(selected) = self.selected_corpus_node {
            let keys = self
                .corpus_graph
                .get_node_annos()
                .get_all_keys_for_item(&selected, None, None);
            let keys = self
                .notifier
                .unwrap_or_default(keys.context("Could not get annotation keys"));

            let text_style_body = egui::TextStyle::Body.resolve(ui.style());
            egui_extras::TableBuilder::new(ui)
                .columns(Column::auto(), 2)
                .columns(Column::remainder(), 1)
                .striped(true)
                .header(text_style_body.size, |mut header| {
                    header.col(|ui| {
                        ui.label("Namespace");
                    });
                    header.col(|ui| {
                        ui.label("Name");
                    });
                    header.col(|ui| {
                        ui.label("Value");
                    });
                })
                .body(|mut body| {
                    for k in keys.iter() {
                        body.row(text_style_body.size, |mut row| {
                            row.col(|ui| {
                                ui.label(k.ns.to_string());
                            });
                            row.col(|ui| {
                                ui.label(k.name.to_string());
                            });
                            row.col(|ui| {
                                let value = self
                                    .corpus_graph
                                    .get_node_annos()
                                    .get_value_for_item(&selected, k);
                                let value = self.notifier.unwrap_or_default(
                                    value.context("Could not get node annotation value"),
                                );
                                ui.label(value.unwrap_or_default());
                            });
                        });
                    }
                });
        } else {
            ui.label("Select a corpus/document node to edit it.");
        }
    }

    pub(crate) fn show(&mut self, ui: &mut Ui) {
        ui.group(|ui| {
            ui.heading("Corpus editor");
            ui.columns_const(|[c1, c2]| {
                c1.push_id("corpus_structure", |ui| self.show_structure(ui));
                c2.push_id("meta_editor", |ui| self.show_meta_editor(ui));
            });
        });
    }
    fn recursive_corpus_structure(&mut self, ui: &mut Ui, parent: NodeID, level: usize) {
        let child_nodes: graphannis_core::errors::Result<Vec<NodeID>> =
            self.gs.get_outgoing_edges(parent).collect();
        let child_nodes = self
            .notifier
            .unwrap_or_default(child_nodes.context("Could not get child nodes"));
        let parent_node_name = self
            .corpus_graph
            .get_node_annos()
            .get_value_for_item(&parent, &NODE_NAME_KEY);
        let parent_node_name = match parent_node_name {
            Ok(o) => o,
            Err(e) => {
                self.notifier.handle_error(e.into());
                None
            }
        };

        if let Some(parent_node_name) = parent_node_name {
            if child_nodes.is_empty() {
                let is_selected = self.selected_corpus_node.is_some_and(|n| n == parent);
                if ui.selectable_label(is_selected, parent_node_name).clicked() {
                    if is_selected {
                        self.selected_corpus_node = None;
                    } else {
                        self.selected_corpus_node = Some(parent);
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
