use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use super::views::Editor;
use crate::app::util::token_helper::{TokenHelper, TOKEN_KEY};
use egui::{
    mutex::RwLock, Align2, Color32, FontId, Label, Pos2, Rangef, Rect, Response, RichText,
    Rounding, ScrollArea, Ui, Widget,
};
use graphannis::{
    graph::{AnnoKey, NodeID},
    model::AnnotationComponentType,
    AnnotationGraph,
};
use graphannis_core::graph::{ANNIS_NS, NODE_NAME_KEY};
use lazy_static::lazy_static;

#[cfg(test)]
mod tests;

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
struct Token {
    start: usize,
    end: usize,
    labels: BTreeMap<AnnoKey, String>,
}

fn make_whitespace_visible(v: &str) -> String {
    v.replace(' ', "␣").replace('\n', "↵")
}

impl Token {
    fn value(&self) -> String {
        if let Some(v) = &self.labels.get(&TOKEN_KEY) {
            make_whitespace_visible(v)
        } else {
            String::default()
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
    segmentations: BTreeMap<String, Vec<Token>>,
    min_token_width: Vec<Option<f32>>,
}

impl DocumentEditor {
    pub fn create_from_graph(
        selected_corpus_node: NodeID,
        graph: Arc<RwLock<AnnotationGraph>>,
    ) -> anyhow::Result<Self> {
        let mut token = Vec::new();
        let mut min_token_width = Vec::new();
        let mut segmentations = BTreeMap::new();

        {
            let graph = graph.read();
            let tok_helper = TokenHelper::new(&graph)?;
            let parent_name = graph
                .get_node_annos()
                .get_value_for_item(&selected_corpus_node, &NODE_NAME_KEY)?
                .unwrap_or_default();
            let mut token_to_index = HashMap::new();
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
                min_token_width.push(None);
                token_to_index.insert(node_id, idx);
            }

            // Find all ordering components othe than the base layer
            for ordering_component in
                graph.get_all_components(Some(AnnotationComponentType::Ordering), None)
            {
                if ordering_component.layer != ANNIS_NS || !ordering_component.name.is_empty() {
                    let token_ids = tok_helper
                        .get_ordered_token(&parent_name, Some(&ordering_component.name))?;
                    for node_id in token_ids.iter() {
                        let mut labels = BTreeMap::new();
                        for anno in graph.get_node_annos().get_annotations_for_item(node_id)? {
                            labels.insert(anno.key, anno.val.to_string());
                        }
                        let covered = tok_helper.covered_token(*node_id)?;
                        let start = covered.first().and_then(|t| token_to_index.get(t));
                        let end = covered.last().and_then(|t| token_to_index.get(t));
                        if let (Some(start), Some(end)) = (start, end) {
                            let t = Token {
                                labels,
                                start: *start,
                                end: *end,
                            };
                            segmentations
                                .entry(ordering_component.name.to_string())
                                .or_insert_with(|| Vec::default())
                                .push(t);
                        }
                    }
                }
            }
        }
        Ok(Self {
            token,
            min_token_width,
            segmentations,
        })
    }

    fn show_single_token(&self, t: &Token, ui: &mut Ui) -> Response {
        let group_response = ui.group(|ui| {
            if let Some(min_width) = self.min_token_width[t.start] {
                ui.set_min_width(min_width);
            }
            ui.vertical(|ui| {
                // Add the token information as first line
                ui.horizontal(|ui| {
                    let token_range = if t.start == t.end {
                        t.start.to_string()
                    } else {
                        format!("{}-{}", t.start, t.end)
                    };
                    ui.label(RichText::new(token_range).weak().small())
                });
                let whitespace_before = t.whitespace_before();
                let whitespace_after = t.whitespace_after();
                let value = t.value();
                if !value.is_empty()
                    || !whitespace_before.is_empty()
                    || !whitespace_after.is_empty()
                {
                    ui.horizontal(|ui| {
                        // Put the whitespace and the actual value in one line
                        if !whitespace_before.is_empty() {
                            ui.label(RichText::new(whitespace_before).weak());
                        }
                        ui.label(RichText::new(t.value()).strong());
                        if !whitespace_after.is_empty() {
                            ui.label(RichText::new(whitespace_after).weak());
                        }
                    });
                }
                // Show all other labels
                for (key, value) in t.labels.iter() {
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

impl Editor for DocumentEditor {
    fn show(&mut self, ui: &mut Ui) {
        let ui_style = ui.style().clone();
        let text_style_body = egui::TextStyle::Body.resolve(&ui_style);
        let span_height = text_style_body.size * 1.5;
        let mut current_span_offset: f32 = 0.0;

        let mut token_offset_to_rect = HashMap::new();
        ScrollArea::both().show(ui, |ui| {
            ui.horizontal(|ui| {
                for t in &self.token {
                    let response = self.show_single_token(t, ui);
                    let token_rect = response.rect;
                    current_span_offset = current_span_offset.max(token_rect.bottom());
                    token_offset_to_rect.insert(t.start, token_rect);
                }
            });
            current_span_offset += ui_style.spacing.item_spacing.y;

            ui.vertical(|ui| {
                for (_segmentation_name, seg_token) in &self.segmentations {
                    for t in seg_token.iter() {
                        let span_value = t.value();

                        // Get the base token covered by this span and use them to create a rectangle
                        let mut covered_span = Rangef::NOTHING;
                        for offset in t.start..=t.end {
                            if let Some(token_rect) = token_offset_to_rect.get(&offset) {
                                covered_span.min = covered_span.min.min(token_rect.left());
                                covered_span.max = covered_span.max.max(token_rect.right());
                            }
                        }
                        if covered_span.span() > 0.0 {
                            let min_pos = Pos2::new(covered_span.min, current_span_offset);
                            let max_pos =
                                Pos2::new(covered_span.max, current_span_offset + span_height);
                            let segmentation_rectangle = Rect::from_min_max(min_pos, max_pos);
                            ui.painter().rect_filled(
                                segmentation_rectangle,
                                Rounding::ZERO,
                                Color32::DARK_GRAY,
                            );

                            let actual_text_rect = ui.painter().text(
                                segmentation_rectangle.center(),
                                Align2::CENTER_CENTER,
                                span_value,
                                FontId::proportional(text_style_body.size),
                                Color32::WHITE,
                            );
                            let span_text_width =
                                actual_text_rect.width() / ((t.end - t.start) as f32 + 1.0);
                            for offset in t.start..=t.end {
                                if let Some(existing) = &mut self.min_token_width[offset] {
                                    self.min_token_width[offset] =
                                        Some(existing.max(span_text_width));
                                } else {
                                    self.min_token_width[offset] = Some(span_text_width);
                                }
                            }
                        }
                    }
                    ui.add_space(span_height + ui_style.spacing.item_spacing.y);
                    current_span_offset += span_height + ui_style.spacing.item_spacing.y;
                }
                // Add additional space for the scrollbar
                ui.add_space(10.0);
            });
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
