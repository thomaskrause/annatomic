use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use egui::{Label, RichText, Widget};
use graphannis::{
    graph::{AnnoKey, NodeID},
    AnnotationGraph,
};
use graphannis_core::graph::ANNIS_NS;
use lazy_static::lazy_static;

use super::util::{make_whitespace_visible, token_helper::TOKEN_KEY};

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

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Token {
    pub node_id: NodeID,
    pub start: usize,
    pub end: usize,
    pub labels: BTreeMap<AnnoKey, String>,
}
impl Token {
    pub fn from_graph(
        node_id: NodeID,
        start: usize,
        end: usize,
        graph: &AnnotationGraph,
    ) -> Result<Self> {
        let mut labels = BTreeMap::new();
        for anno in graph.get_node_annos().get_annotations_for_item(&node_id)? {
            labels.insert(anno.key, anno.val.to_string());
        }
        Ok(Token {
            node_id,
            start,
            end,
            labels,
        })
    }
}

#[derive(Debug)]
pub struct TokenEditor<'t> {
    token: &'t Token,
    min_width: Option<f32>,
    value: String,
    whitespace_before: String,
    whitespace_after: String,
}

impl<'t> TokenEditor<'t> {
    pub fn new(token: &'t Token, min_width: Option<f32>) -> Self {
        TokenEditor {
            token,
            min_width,
            value: token
                .labels
                .get(&TOKEN_KEY)
                .map(make_whitespace_visible)
                .unwrap_or_default(),
            whitespace_before: token
                .labels
                .get(&WITESPACE_BEFORE)
                .map(make_whitespace_visible)
                .unwrap_or_default(),
            whitespace_after: token
                .labels
                .get(&WITESPACE_AFTER)
                .map(make_whitespace_visible)
                .unwrap_or_default(),
        }
    }
}

impl<'t> Widget for TokenEditor<'t> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let group_response = ui.group(|ui| {
            if let Some(min_width) = self.min_width {
                ui.set_min_width(min_width);
            }

            ui.vertical(|ui| {
                // Add the token information as first line
                ui.horizontal(|ui| {
                    let token_range = if self.token.start == self.token.end {
                        self.token.start.to_string()
                    } else {
                        format!("{}-{}", self.token.start, self.token.end)
                    };
                    ui.label(RichText::new(token_range).weak().small())
                });
                if !self.value.is_empty()
                    || !self.whitespace_before.is_empty()
                    || !self.whitespace_after.is_empty()
                {
                    ui.horizontal(|ui| {
                        // Put the whitespace and the actual value in one line
                        if !self.whitespace_before.is_empty() {
                            ui.label(RichText::new(&self.whitespace_before).weak());
                        }
                        ui.label(RichText::new(&self.value).strong());
                        if !self.whitespace_after.is_empty() {
                            ui.label(RichText::new(&self.whitespace_after).weak());
                        }
                    });
                }
                // Show all other labels
                for (key, value) in self.token.labels.iter() {
                    if key.ns != ANNIS_NS {
                        let key_label = if key.ns.is_empty() {
                            key.name.to_string()
                        } else {
                            format!("{}:{}", key.ns, key.name)
                        };

                        ui.horizontal(|ui| {
                            Label::new(value)
                                .wrap_mode(egui::TextWrapMode::Extend)
                                .ui(ui);
                            Label::new(RichText::new(key_label).weak().small_raised())
                                .wrap_mode(egui::TextWrapMode::Extend)
                                .ui(ui);
                        });
                    }
                }
            });
        });
        group_response.response
    }
}
