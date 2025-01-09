use std::{collections::BTreeMap, sync::Arc};

use super::views::Editor;
use crate::app::util::token_helper::{TokenHelper, TOKEN_KEY};
use egui::{mutex::RwLock, RichText, ScrollArea, Ui};
use graphannis::{
    graph::{AnnoKey, NodeID},
    AnnotationGraph,
};
use graphannis_core::graph::{ANNIS_NS, NODE_NAME_KEY};
use lazy_static::lazy_static;

lazy_static! {
    static ref WITESPACE_BEFORE: Arc<AnnoKey> = Arc::from(AnnoKey {
        ns: ANNIS_NS.into(),
        name: "tok-whitespace-before".into(),
    });
    static ref WITESPACE_AFTER: Arc<AnnoKey> = Arc::from(AnnoKey {
        ns: ANNIS_NS.into(),
        name: "tok-whitespace-after".into(),
    });
}

struct Token {
    start: usize,
    end: usize,
    labels: BTreeMap<AnnoKey, String>,
}

fn make_whitespace_visible(v: &str) -> String {
    v.replace(' ', "␣").replace('\n', "↵")
}

impl Token {
    fn value(&self) -> &str {
        if let Some(v) = &self.labels.get(&TOKEN_KEY) {
            v.as_str()
        } else {
            ""
        }
    }

    fn whitespace_before(&self) -> String {
        if let Some(v) = &self.labels.get(&WITESPACE_BEFORE) {
            make_whitespace_visible(v)
        } else {
            String::default()
        }
    }
    fn whitespace_after(&self) -> String {
        if let Some(v) = &self.labels.get(&WITESPACE_AFTER) {
            make_whitespace_visible(v)
        } else {
            String::default()
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
            for (idx, node_id) in token_ids.iter().enumerate() {
                let mut labels = BTreeMap::new();
                for anno in graph.get_node_annos().get_annotations_for_item(node_id)? {
                    labels.insert(anno.key, anno.val.to_string());
                }
                let t = Token {
                    labels,
                    start: idx,
                    end: idx,
                };
                token.push(t);
            }
        }
        Ok(Self { token })
    }
}

impl Editor for DocumentEditor {
    fn show(&mut self, ui: &mut Ui) {
        ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for t in &self.token {
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            // Add the token information as first line
                            ui.horizontal_top(|ui| {
                                let token_range = if t.start == t.end {
                                    t.start.to_string()
                                } else {
                                    format!("{}-{}", t.start, t.end)
                                };
                                ui.label(RichText::new(token_range).weak().small())
                            });
                            ui.horizontal(|ui| {
                                // Put the whitespace and the actual value in one line
                                let whitespace_before = t.whitespace_before();
                                if !whitespace_before.is_empty() {
                                    ui.label(RichText::new(whitespace_before).weak());
                                }
                                ui.label(RichText::new(t.value()).strong());
                                let whitespace_after = t.whitespace_after();
                                if !whitespace_after.is_empty() {
                                    ui.label(RichText::new(whitespace_after).weak());
                                }
                            });
                        });
                    });
                }
            });

            ui.add_space(10.0);
        });
    }

    fn has_pending_updates(&self) -> bool {
        false
    }

    fn apply_pending_updates(&mut self) {}

    fn get_selected_corpus_node(&self) -> Option<NodeID> {
        None
    }
}
