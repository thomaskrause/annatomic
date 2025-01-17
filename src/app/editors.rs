use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use super::{views::Editor, JobExecutor};
use crate::app::util::token_helper::{TokenHelper, TOKEN_KEY};
use anyhow::{Context, Result};
use egui::{
    mutex::RwLock, Button, Color32, FontId, Label, Pos2, Rangef, Rect, Response, RichText,
    ScrollArea, TextEdit, Ui, Widget,
};
use graphannis::{
    graph::{AnnoKey, NodeID},
    model::AnnotationComponentType,
    update::{GraphUpdate, UpdateEvent},
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

fn make_whitespace_visible<S: AsRef<str>>(v: &S) -> String {
    v.as_ref().replace(' ', "␣").replace('\n', "↵")
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct DisplayedToken {
    value: String,
    whitespace_before: String,
    whitespace_after: String,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct Token {
    node_id: NodeID,
    start: usize,
    end: usize,
    labels: BTreeMap<AnnoKey, String>,
    displayed: DisplayedToken,
}

impl Token {
    fn from_graph(
        node_id: NodeID,
        start: usize,
        end: usize,
        graph: &AnnotationGraph,
    ) -> Result<Self> {
        let mut labels = BTreeMap::new();
        for anno in graph.get_node_annos().get_annotations_for_item(&node_id)? {
            labels.insert(anno.key, anno.val.to_string());
        }
        let displayed = DisplayedToken {
            value: labels
                .get(&TOKEN_KEY)
                .map(make_whitespace_visible)
                .unwrap_or_default(),
            whitespace_before: labels
                .get(&WITESPACE_BEFORE)
                .map(make_whitespace_visible)
                .unwrap_or_default(),
            whitespace_after: labels
                .get(&WITESPACE_AFTER)
                .map(make_whitespace_visible)
                .unwrap_or_default(),
        };
        Ok(Token {
            node_id,
            start,
            end,
            labels,
            displayed,
        })
    }
}

struct LayoutInfo {
    valid: bool,
    min_token_width: Vec<f32>,
    token_offset_start: Vec<f32>,
    token_offset_end: Vec<f32>,
}

pub(crate) struct DocumentEditor {
    graph: Arc<RwLock<AnnotationGraph>>,
    token: Vec<Token>,
    selected_node: Option<NodeID>,
    current_edited_value: String,
    segmentations: BTreeMap<String, Vec<Token>>,
    layout_info: LayoutInfo,
    jobs: JobExecutor,
}

impl DocumentEditor {
    pub fn create_from_graph(
        selected_corpus_node: NodeID,
        graph: Arc<RwLock<AnnotationGraph>>,
        jobs: JobExecutor,
    ) -> Result<Self> {
        let mut token = Vec::new();
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
                let t = Token::from_graph(*node_id, idx, idx, &graph)?;
                token.push(t);
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
                        let covered = tok_helper.covered_token(*node_id)?;
                        let start = covered.first().and_then(|t| token_to_index.get(t));
                        let end = covered.last().and_then(|t| token_to_index.get(t));
                        if let (Some(start), Some(end)) = (start, end) {
                            let t = Token::from_graph(*node_id, *start, *end, &graph)?;

                            segmentations
                                .entry(ordering_component.name.to_string())
                                .or_insert_with(Vec::default)
                                .push(t);
                        }
                    }
                }
            }
        }
        let nr_token = token.len();
        Ok(Self {
            graph,
            token,
            layout_info: LayoutInfo {
                valid: false,
                min_token_width: Vec::new(),
                token_offset_start: vec![0.0; nr_token],
                token_offset_end: vec![0.0; nr_token],
            },
            segmentations,
            selected_node: None,
            current_edited_value: String::new(),
            jobs,
        })
    }

    fn show_single_token(&self, t: &Token, ui: &mut Ui) -> Response {
        let group_response = ui.group(|ui| {
            if let Some(min_width) = self.layout_info.min_token_width.get(t.start) {
                ui.set_min_width(*min_width);
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
                let displayed = &t.displayed;
                if !displayed.value.is_empty()
                    || !displayed.whitespace_before.is_empty()
                    || !displayed.whitespace_after.is_empty()
                {
                    ui.horizontal(|ui| {
                        // Put the whitespace and the actual value in one line
                        if !t.displayed.whitespace_before.is_empty() {
                            ui.label(RichText::new(&displayed.whitespace_before).weak());
                        }
                        ui.label(RichText::new(&displayed.value).strong());
                        if !t.displayed.whitespace_after.is_empty() {
                            ui.label(RichText::new(&displayed.whitespace_after).weak());
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

        // Remember the location of each token, so we can paint the spans with
        // the same range later
        let mut token_offset_to_rect = vec![None; self.token.len()];
        ScrollArea::horizontal().show_viewport(ui, |ui, visible_rect| {
            if !self.layout_info.valid {
                ui.scroll_to_cursor(Some(egui::Align::LEFT));
            }
            // If we already calculated the token positions once, only render
            // the token and their covering spans that are currently displayed
            let mut first_visible_token: usize = 0;
            let last_token_index = self.token.len() - 1;
            let mut last_visible_token: usize = last_token_index;
            let visible_range = visible_rect.x_range().min..visible_rect.x_range().max;
            if self.layout_info.valid {
                first_visible_token = self
                    .layout_info
                    .token_offset_start
                    .partition_point(|x| {
                        x.partial_cmp(&visible_range.start)
                            .unwrap_or(Ordering::Equal)
                            .is_lt()
                    })
                    .saturating_sub(1);
                last_visible_token = self
                    .layout_info
                    .token_offset_end
                    .partition_point(|x| {
                        x.partial_cmp(&visible_range.end)
                            .unwrap_or(Ordering::Equal)
                            .is_lt()
                    })
                    .saturating_add(1);
            }
            if last_visible_token > last_token_index {
                last_visible_token = last_token_index
            }

            ui.horizontal(|ui| {
                if self.layout_info.valid && first_visible_token > 0 {
                    // Add the space needed for the non-rendered token at the beginning
                    ui.add_space(self.layout_info.token_offset_end[first_visible_token - 1]);
                }

                for t in &self.token[first_visible_token..=last_visible_token] {
                    let response = self.show_single_token(t, ui);
                    let token_rect = response.rect;
                    current_span_offset = current_span_offset.max(token_rect.bottom());
                    token_offset_to_rect[t.start] = Some(token_rect);

                    if !self.layout_info.valid {
                        let offset_range = token_rect.x_range();
                        self.layout_info.token_offset_start[t.start] = offset_range.min;
                        self.layout_info.token_offset_end[t.start] = offset_range.max;
                    }
                }
                if self.layout_info.valid && last_visible_token < last_token_index {
                    // Add the space needed for the non-rendered token at the end
                    let visible_token_end = self.layout_info.token_offset_end[last_visible_token];
                    let last_token_end = self.layout_info.token_offset_end[last_token_index];
                    let space = last_token_end - visible_token_end;

                    if space > 0.0 {
                        ui.add_space(space);
                    }
                }
            });
            current_span_offset += ui_style.spacing.item_spacing.y;

            if self.layout_info.min_token_width.is_empty() {
                self.layout_info.min_token_width = vec![0.0; self.token.len()];
            }

            ui.vertical(|ui| {
                for (_, seg_token) in self.segmentations.iter_mut() {
                    for t in seg_token.iter_mut() {
                        let span_value = t.displayed.value.clone();

                        // Get the base token covered by this span and use them to create a rectangle
                        let mut covered_span = Rangef::NOTHING;
                        for token_rect in token_offset_to_rect
                            .iter()
                            .take(t.end + 1)
                            .skip(t.start)
                            .flatten()
                        {
                            covered_span.min = covered_span.min.min(token_rect.left());
                            covered_span.max = covered_span.max.max(token_rect.right());
                        }
                        if covered_span.span() > 0.0 {
                            let min_pos = Pos2::new(covered_span.min, current_span_offset);
                            let max_pos =
                                Pos2::new(covered_span.max, current_span_offset + span_height);
                            let segmentation_rectangle = Rect::from_min_max(min_pos, max_pos);

                            if ui.is_rect_visible(segmentation_rectangle) {
                                if self.selected_node == Some(t.node_id) {
                                    let span_editor =
                                        TextEdit::singleline(&mut self.current_edited_value);
                                    if ui.put(segmentation_rectangle, span_editor).lost_focus() {
                                        // TODO: apply this change

                                        self.selected_node = None;
                                        let new_value = self.current_edited_value.clone();
                                        let old_value = t.labels.get(&TOKEN_KEY);
                                        if Some(&new_value) != old_value {
                                            t.displayed.value = make_whitespace_visible(&new_value);
                                            t.labels.insert(
                                                TOKEN_KEY.as_ref().clone(),
                                                new_value.clone(),
                                            );

                                            self.layout_info.valid = false;
                                            let graph = self.graph.clone();
                                            let node_id = t.node_id;
                                            self.jobs.add(
                                                "Applying segmentation value change",
                                                move |_job| {
                                                    let graph = graph.read();
                                                    let node_name = graph
                                                        .get_node_annos()
                                                        .get_value_for_item(
                                                            &node_id,
                                                            &NODE_NAME_KEY,
                                                        )?
                                                        .context("Missing node name")?;

                                                    let mut updates = GraphUpdate::new();
                                                    updates.add_event(
                                                        UpdateEvent::DeleteNodeLabel {
                                                            node_name: node_name.to_string(),
                                                            anno_ns: TOKEN_KEY.ns.clone().into(),
                                                            anno_name: TOKEN_KEY
                                                                .name
                                                                .clone()
                                                                .into(),
                                                        },
                                                    )?;
                                                    updates.add_event(
                                                        UpdateEvent::AddNodeLabel {
                                                            node_name: node_name.to_string(),
                                                            anno_ns: TOKEN_KEY.ns.clone().into(),
                                                            anno_name: TOKEN_KEY
                                                                .name
                                                                .clone()
                                                                .into(),
                                                            anno_value: new_value,
                                                        },
                                                    )?;
                                                    Ok(updates)
                                                },
                                                |update, app| {
                                                    app.project.add_changeset(update);
                                                    app.apply_pending_updates();
                                                },
                                            );
                                        }
                                    }
                                } else {
                                    let span_button = Button::new(&span_value)
                                        .wrap_mode(egui::TextWrapMode::Truncate);
                                    if ui.put(segmentation_rectangle, span_button).clicked() {
                                        self.selected_node = Some(t.node_id);
                                        self.current_edited_value =
                                            t.labels.get(&TOKEN_KEY).cloned().unwrap_or_default();
                                    }
                                }

                                let actual_text_rect = ui
                                    .painter()
                                    .layout_no_wrap(
                                        span_value.clone(),
                                        FontId::proportional(text_style_body.size),
                                        Color32::BLACK,
                                    )
                                    .rect;

                                let span_text_width =
                                    actual_text_rect.width() / ((t.end - t.start) as f32 + 1.0);
                                for offset in t.start..=t.end {
                                    if let Some(existing) =
                                        self.layout_info.min_token_width.get(offset)
                                    {
                                        self.layout_info.min_token_width[offset] =
                                            existing.max(span_text_width);
                                    } else {
                                        self.layout_info.min_token_width[offset] = span_text_width;
                                    }
                                }
                            }
                        }
                    }
                    current_span_offset += span_height + ui_style.spacing.item_spacing.y;
                }
                // Add additional space for the scrollbar
                ui.add_space(10.0);
            });
            if visible_range.start == 0.0 && !self.layout_info.min_token_width.is_empty() {
                self.layout_info.valid = true;
            }
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
