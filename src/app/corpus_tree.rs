use std::{borrow::Cow, sync::Arc};

use anyhow::Context;
use egui::{CollapsingHeader, ScrollArea, Ui};
use graphannis::{
    graph::{Edge, EdgeContainer, GraphStorage, NodeID, WriteableGraphStorage},
    model::{AnnotationComponent, AnnotationComponentType::PartOf},
    AnnotationGraph, CorpusStorage,
};
use graphannis_core::graph::{
    storage::{adjacencylist::AdjacencyListStorage, prepost::PrePostOrderStorage},
    ANNIS_NS, NODE_NAME_KEY,
};

pub(crate) struct CorpusTree {
    gs: Box<dyn GraphStorage>,
    corpus_graph: AnnotationGraph,
}

impl CorpusTree {
    pub fn create_from_graphstorage(
        cs: Arc<CorpusStorage>,
        corpus_name: &str,
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
                inverted_corpus_graph.add_edge(Edge { source, target }.inverse())?;
            }
        }
        inverted_corpus_graph.calculate_statistics()?;
        let gs: Box<dyn GraphStorage> = if let Some(stats) = inverted_corpus_graph.get_statistics()
        {
            if !stats.cyclic && stats.rooted_tree {
                // Use an optimized implementation for trees
                let mut optimized_corpus_graph = PrePostOrderStorage::<u64, u64>::new();
                optimized_corpus_graph
                    .copy(corpus_graph.get_node_annos(), &inverted_corpus_graph)?;
                Box::new(optimized_corpus_graph)
            } else {
                Box::new(inverted_corpus_graph)
            }
        } else {
            Box::new(inverted_corpus_graph)
        };

        Ok(Self { gs, corpus_graph })
    }

    pub(crate) fn show(&self, ui: &mut Ui) -> anyhow::Result<()> {
        ui.heading("Corpus editor");
        let root_nodes: graphannis_core::errors::Result<Vec<_>> = self.gs.root_nodes().collect();
        let root_nodes = root_nodes?;
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

        Ok(())
    }
    fn recursive_corpus_structure(&self, ui: &mut Ui, parent: NodeID, level: usize) {
        let child_nodes: graphannis_core::errors::Result<Vec<NodeID>> =
            self.gs.get_outgoing_edges(parent).collect();
        let parent_node_name = self
            .corpus_graph
            .get_node_annos()
            .get_value_for_item(&parent, &NODE_NAME_KEY);

        if let (Ok(child_nodes), Ok(parent_node_name)) = (child_nodes, parent_node_name) {
            let parent_node_name =
                parent_node_name.unwrap_or(Cow::Borrowed("<node name not found>"));
            if child_nodes.is_empty() {
                if ui.selectable_label(false, parent_node_name).clicked() {
                    // TODO
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
        }
    }
}
