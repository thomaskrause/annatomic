use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use super::{
    util::make_whitespace_visible,
    views::Editor,
    widgets::{Token, TokenEditor},
    JobExecutor,
};
use crate::app::util::token_helper::{TokenHelper, TOKEN_KEY};
use anyhow::{Context, Result};
use egui::{
    mutex::RwLock, Button, Color32, FontId, Key, KeyboardShortcut, Modifiers, Pos2, Rangef, Rect,
    ScrollArea, TextEdit, Ui, Widget,
};
use graphannis::{
    graph::NodeID,
    model::AnnotationComponentType,
    update::{GraphUpdate, UpdateEvent},
    AnnotationGraph,
};
use graphannis_core::graph::{ANNIS_NS, NODE_NAME_KEY};

#[cfg(test)]
mod tests;

const DELETE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Delete);

struct LayoutInfo {
    valid: bool,
    first_frame: bool,
    min_token_width: Vec<f32>,
    token_offset_start: Vec<f32>,
    token_offset_end: Vec<f32>,
}

enum EditorActions {
    ModifySegmentationValue { node_id: NodeID, new_value: String },
    DeleteNode { node_id: NodeID },
}

pub(crate) struct DocumentEditor {
    graph: Arc<RwLock<AnnotationGraph>>,
    token: Vec<Token>,
    selected_nodes: HashSet<NodeID>,
    currently_edited_node: Option<NodeID>,
    current_edited_value: String,
    pending_actions: Vec<EditorActions>,
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
                first_frame: true,
                min_token_width: Vec::new(),
                token_offset_start: vec![0.0; nr_token],
                token_offset_end: vec![0.0; nr_token],
            },
            segmentations,
            selected_nodes: HashSet::new(),
            pending_actions: Vec::new(),
            currently_edited_node: None,
            current_edited_value: String::new(),
            jobs,
        })
    }

    fn show_segmentation_layers(
        &mut self,
        ui: &mut Ui,
        token_offset_to_rect: &mut Vec<Option<Rect>>,
        mut current_span_offset: f32,
        span_height: f32,
    ) {
        let ui_style = ui.style().clone();
        let text_style_body = egui::TextStyle::Body.resolve(&ui_style);
        for (_, seg_token) in self.segmentations.iter_mut() {
            for t in seg_token.iter_mut() {
                let span_value_raw = t
                    .labels
                    .get(&TOKEN_KEY)
                    .map(|s| s.as_str())
                    .unwrap_or_default();
                let span_value = make_whitespace_visible(span_value_raw);

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
                    let max_pos = Pos2::new(covered_span.max, current_span_offset + span_height);
                    let segmentation_rectangle = Rect::from_min_max(min_pos, max_pos);

                    if ui.is_rect_visible(segmentation_rectangle) {
                        if self.currently_edited_node == Some(t.node_id) {
                            let span_editor = TextEdit::singleline(&mut self.current_edited_value);
                            if ui.put(segmentation_rectangle, span_editor).lost_focus() {
                                self.currently_edited_node = None;
                                self.selected_nodes.remove(&t.node_id);
                                let new_value = self.current_edited_value.clone();
                                let old_value = t.labels.get(&TOKEN_KEY);
                                if Some(&new_value) != old_value {
                                    t.labels
                                        .insert(TOKEN_KEY.as_ref().clone(), new_value.clone());

                                    self.layout_info.valid = false;
                                    self.pending_actions.push(
                                        EditorActions::ModifySegmentationValue {
                                            node_id: t.node_id,
                                            new_value: new_value.clone(),
                                        },
                                    );
                                }
                            }
                        } else {
                            let button_selected = self.selected_nodes.contains(&t.node_id);
                            let span_button = Button::new(&span_value)
                                .selected(button_selected)
                                .wrap_mode(egui::TextWrapMode::Truncate);
                            let span_button = ui.put(segmentation_rectangle, span_button);
                            if span_button.clicked() {
                                if button_selected {
                                    // Already selected, allow editing
                                    self.currently_edited_node = Some(t.node_id);
                                    self.current_edited_value =
                                        t.labels.get(&TOKEN_KEY).cloned().unwrap_or_default();
                                } else {
                                    if !ui.ctx().input(|i| i.modifiers.command) {
                                        // Select only one item unless Ctrl/Cmd key is down
                                        self.selected_nodes.clear();
                                    }
                                    // Select first before it can be edited
                                    self.selected_nodes.insert(t.node_id);
                                }
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
                            if let Some(existing) = self.layout_info.min_token_width.get(offset) {
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
    }

    fn delete_selected_nodes(&mut self) {
        self.layout_info.valid = false;
        for (_, segmentation_token) in self.segmentations.iter_mut() {
            segmentation_token.retain(|t| !self.selected_nodes.contains(&t.node_id));
        }
        for n in self.selected_nodes.iter() {
            self.pending_actions
                .push(EditorActions::DeleteNode { node_id: *n });
        }
        self.selected_nodes.clear();
        self.apply_pending_updates();
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
            if self.layout_info.first_frame {
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
                    let response =
                        TokenEditor::new(t, self.layout_info.min_token_width.get(t.start).copied())
                            .ui(ui);
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
                self.show_segmentation_layers(
                    ui,
                    &mut token_offset_to_rect,
                    current_span_offset,
                    span_height,
                )
            });

            // Add additional space for the scrollbar
            ui.add_space(10.0);

            if visible_range.start == 0.0 && !self.layout_info.min_token_width.is_empty() {
                self.layout_info.valid = true;
            }
            self.apply_pending_updates();
        });

        self.layout_info.first_frame = false;
    }

    fn has_pending_updates(&self) -> bool {
        !self.pending_actions.is_empty()
    }

    fn apply_pending_updates(&mut self) {
        if !self.has_pending_updates() {
            return;
        }
        let graph = self.graph.clone();
        let pending_actions = std::mem::take(&mut self.pending_actions);
        self.jobs.add(
            "Applying segmentation value change",
            move |_job| {
                let mut updates = GraphUpdate::new();
                let graph = graph.read();

                for action in pending_actions {
                    match action {
                        EditorActions::ModifySegmentationValue { node_id, new_value } => {
                            let node_name = graph
                                .get_node_annos()
                                .get_value_for_item(&node_id, &NODE_NAME_KEY)?
                                .context("Missing node name")?;

                            updates.add_event(UpdateEvent::DeleteNodeLabel {
                                node_name: node_name.to_string(),
                                anno_ns: TOKEN_KEY.ns.clone().into(),
                                anno_name: TOKEN_KEY.name.clone().into(),
                            })?;
                            updates.add_event(UpdateEvent::AddNodeLabel {
                                node_name: node_name.to_string(),
                                anno_ns: TOKEN_KEY.ns.clone().into(),
                                anno_name: TOKEN_KEY.name.clone().into(),
                                anno_value: new_value,
                            })?;
                        }
                        EditorActions::DeleteNode { node_id } => {
                            let node_name = graph
                                .get_node_annos()
                                .get_value_for_item(&node_id, &NODE_NAME_KEY)?
                                .context("Missing node name")?;
                            updates.add_event(UpdateEvent::DeleteNode {
                                node_name: node_name.to_string(),
                            })?;
                        }
                    }
                }

                Ok(updates)
            },
            |update, app| {
                app.project.add_changeset(update);
                app.apply_pending_updates();
            },
        );
    }

    fn get_selected_corpus_node(&self) -> Option<NodeID> {
        None
    }

    fn consume_shortcuts(&mut self, ctx: &egui::Context) {
        if !self.selected_nodes.is_empty()
            && self.currently_edited_node.is_none()
            && ctx.input_mut(|i| i.consume_shortcut(&DELETE_SHORTCUT))
        {
            self.delete_selected_nodes();
        }
    }

    fn add_edit_menu_entries(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(
                !self.selected_nodes.is_empty(),
                Button::new("Delete selected")
                    .shortcut_text(ui.ctx().format_shortcut(&DELETE_SHORTCUT)),
            )
            .clicked()
        {
            self.delete_selected_nodes();
        }
    }
}
