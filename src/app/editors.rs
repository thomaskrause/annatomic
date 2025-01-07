use std::{collections::BTreeMap, sync::Arc};

use egui::{mutex::RwLock, Ui};
use graphannis::{
    graph::{AnnoKey, NodeID},
    AnnotationGraph,
};
use graphannis_core::graph::NODE_NAME_KEY;

use crate::util::token_helper::{TokenHelper, TOKEN_KEY};

struct Token {
    labels: BTreeMap<AnnoKey, String>,
}

impl Token {
    fn value(&self) -> &str {
        if let Some(v) = &self.labels.get(&TOKEN_KEY) {
            v.as_str()
        } else {
            ""
        }
    }
}

pub(crate) struct DocumentEditor {
    token: Vec<Token>,
}

impl DocumentEditor {
    pub fn create_from_graph(
        selected_corpus_node: NodeID,
        graph: Arc<RwLock<AnnotationGraph>>,
    ) -> anyhow::Result<Self> {
        let mut token = Vec::new();
        {
            let graph = graph.read();
            let tok_helper = TokenHelper::new(&graph)?;
            let parent_name = graph
                .get_node_annos()
                .get_value_for_item(&selected_corpus_node, &NODE_NAME_KEY)?
                .unwrap_or_default();
            let token_ids = tok_helper.get_ordered_token(&parent_name, None)?;
            for id in token_ids {
                let mut labels = BTreeMap::new();
                for anno in graph.get_node_annos().get_annotations_for_item(&id)? {
                    labels.insert(anno.key, anno.val.to_string());
                }
                let t = Token { labels };
                token.push(t);
            }
        }
        Ok(Self { token })
    }

    pub(crate) fn show(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for t in &self.token {
                ui.label(t.value());
            }
        });
    }
}
