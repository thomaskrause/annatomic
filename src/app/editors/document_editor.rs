use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use crate::app::{
    util::token_helper::{TokenHelper, TOKEN_KEY},
    views::Editor,
    widgets::{Token, TokenEditor},
    JobExecutor,
};
use anyhow::{Context, Result};
use egui::{
    mutex::RwLock, Button, Key, KeyboardShortcut, Modifiers, Pos2, Rangef, Rect, ScrollArea,
    TextEdit, Ui, Widget,
};
use graphannis::{
    graph::{AnnoKey, NodeID},
    model::AnnotationComponentType,
    update::{GraphUpdate, UpdateEvent},
    AnnotationGraph,
};
use graphannis_core::graph::{ANNIS_NS, NODE_NAME_KEY};

#[cfg(test)]
mod tests;

const DELETE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Delete);

#[derive(Clone)]
struct LayoutInfo {
    valid: bool,
    first_frame: bool,
    min_token_width: Vec<f32>,
    token_offset_start: Vec<f32>,
    token_offset_end: Vec<f32>,
}

#[derive(Clone)]
enum EditorActions {
    ModifySegmentationValue {
        node_name: String,
        new_value: String,
    },
    AddSegmentationSpan {
        segmentation: String,
        selected_token: HashSet<String>,
    },
    DeleteNode {
        node_name: String,
    },
}

type StateUpdateFn = Box<dyn FnOnce(&mut DocumentEditor) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct DocumentEditor {
    parent_name: String,
    graph: Arc<RwLock<AnnotationGraph>>,
    token: Vec<Token>,
    token_index_by_name: HashMap<String, usize>,
    selected_nodes: HashSet<String>,
    currently_edited_node: Option<String>,
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
        let parent_name;

        {
            let graph = graph.read();
            let tok_helper = TokenHelper::new(&graph)?;
            parent_name = graph
                .get_node_annos()
                .get_value_for_item(&selected_corpus_node, &NODE_NAME_KEY)?
                .unwrap_or_default()
                .to_string();
            let mut token_to_index = HashMap::new();
            let token_ids = tok_helper.get_ordered_token(&parent_name, None)?;
            for (idx, node_id) in token_ids.iter().enumerate() {
                let t = Token::from_graph(*node_id, idx, idx, &graph)?;
                token.push(t);
                token_to_index.insert(node_id, idx);
            }

            // Find all ordering components other than the base layer
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

        let token_index_by_name = token
            .iter()
            .enumerate()
            .map(|(idx, t)| (t.node_name.clone(), idx))
            .collect();

        Ok(Self {
            parent_name,
            graph,
            token,
            token_index_by_name,
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
        token_offset_to_rect: &[Option<Rect>],
        mut current_span_offset: f32,
    ) {
        let ui_style = ui.style().clone();
        for (_, seg_token) in self.segmentations.iter_mut() {
            let mut max_node_height = 0.0;
            for t in seg_token.iter_mut() {
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
                    let max_pos = Pos2::new(covered_span.max, current_span_offset);
                    let segmentation_rectangle = Rect::from_min_max(min_pos, max_pos);

                    if ui.is_rect_visible(segmentation_rectangle) {
                        if self.currently_edited_node == Some(t.node_name.clone()) {
                            let segmentation_editor =
                                TextEdit::singleline(&mut self.current_edited_value);
                            let segmentation_editor =
                                ui.put(segmentation_rectangle, segmentation_editor);
                            max_node_height =
                                segmentation_editor.rect.height().max(max_node_height);
                            if segmentation_editor.lost_focus() {
                                self.currently_edited_node = None;
                                self.selected_nodes.remove(&t.node_name);
                                let new_value = self.current_edited_value.clone();
                                let old_value = t.labels.get(&TOKEN_KEY);
                                if Some(&new_value) != old_value {
                                    t.labels
                                        .insert(TOKEN_KEY.as_ref().clone(), new_value.clone());

                                    self.layout_info.valid = false;
                                    self.pending_actions.push(
                                        EditorActions::ModifySegmentationValue {
                                            node_name: t.node_name.clone(),
                                            new_value: new_value.clone(),
                                        },
                                    );
                                }
                            }
                        } else {
                            let selected = self.selected_nodes.contains(&t.node_name);
                            let segmentation_editor = TokenEditor::with_exact_width(
                                t,
                                selected,
                                Some(segmentation_rectangle.width()),
                            );

                            let segmentation_editor =
                                ui.put(segmentation_rectangle, segmentation_editor);
                            max_node_height =
                                segmentation_editor.rect.height().max(max_node_height);
                            if segmentation_editor.clicked() {
                                if selected {
                                    // Already selected, allow editing
                                    self.currently_edited_node = Some(t.node_name.clone());
                                    self.current_edited_value =
                                        t.labels.get(&TOKEN_KEY).cloned().unwrap_or_default();
                                } else {
                                    if !ui.ctx().input(|i| i.modifiers.command_only()) {
                                        // Select only one item unless Ctrl/Cmd key is down
                                        self.selected_nodes.clear();
                                    }
                                    // Select first before it can be edited
                                    self.selected_nodes.insert(t.node_name.clone());
                                }
                            }
                            let span_text_width = (segmentation_editor.rect.width()
                                / ((t.end - t.start) as f32 + 1.0))
                                + 5.0;
                            for offset in t.start..=t.end {
                                if offset < self.layout_info.min_token_width.len()
                                    && self.layout_info.min_token_width[offset] == 0.0
                                {
                                    self.layout_info.min_token_width[offset] = span_text_width;
                                }
                            }
                        }
                    }
                }
            }
            current_span_offset += max_node_height + ui_style.spacing.item_spacing.y;
        }
    }

    fn select_range(&mut self, token_position: usize) {
        // Mark a range of token, find a suitable token as start for the range first
        let mut selected_token_indices: BTreeSet<_> = self
            .selected_nodes
            .iter()
            .filter_map(|selected_node| self.token_index_by_name.get(selected_node))
            .copied()
            .collect();
        let after = selected_token_indices.split_off(&token_position);

        if let Some(after) = after.first() {
            for i in token_position..*after {
                self.selected_nodes.insert(self.token[i].node_name.clone());
            }
        } else if let Some(before) = selected_token_indices.last() {
            for i in *before..token_position {
                self.selected_nodes.insert(self.token[i].node_name.clone());
            }
        }
        self.selected_nodes
            .insert(self.token[token_position].node_name.clone());
    }

    /// Adds an empty segmentation node that spans the currently selected token.
    ///
    /// - `layer_idx` The segmentation layer to add the new node to. **Starts with 1.**
    fn add_segmentation_for_selection(&mut self, layer_idx: usize) {
        if let Some((seg_name, _token)) = self.segmentations.iter().nth(layer_idx.saturating_sub(1))
        {
            if !self.selected_nodes.is_empty() {
                // Apply changes to internal data model
                let mut selected_token_indices: Vec<_> = self
                    .selected_nodes
                    .iter()
                    .filter_map(|n| self.token_index_by_name.get(n))
                    .copied()
                    .collect();
                selected_token_indices.sort();
                {
                    let graph = self.graph.read();
                    if let Ok(tok_helper) = TokenHelper::new(&graph) {
                        // Schedule an update of the underlaying graph
                        let selected_token: HashSet<_> = self
                            .selected_nodes
                            .iter()
                            .filter(|node_name| {
                                if let Ok(Some(node_id)) =
                                    graph.get_node_annos().get_node_id_from_name(node_name)
                                {
                                    tok_helper.is_token(node_id).unwrap_or(false)
                                } else {
                                    false
                                }
                            })
                            .cloned()
                            .collect();

                        self.pending_actions
                            .push(EditorActions::AddSegmentationSpan {
                                segmentation: seg_name.clone(),
                                selected_token,
                            });
                    }
                }
                self.apply_pending_updates_for_editor();
            }
        }
    }

    fn delete_selected_nodes(&mut self) {
        self.layout_info.valid = false;
        for (_, segmentation_token) in self.segmentations.iter_mut() {
            segmentation_token.retain(|t| !self.selected_nodes.contains(&t.node_name));
        }
        for n in self.selected_nodes.iter() {
            self.pending_actions.push(EditorActions::DeleteNode {
                node_name: n.clone(),
            });
        }
        self.selected_nodes.clear();
        self.apply_pending_updates_for_editor();
    }
}

impl Editor for DocumentEditor {
    fn show(&mut self, ui: &mut Ui) {
        let ui_style = ui.style().clone();
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

                for token_position in first_visible_token..=last_visible_token {
                    let token_node_name = &self.token[token_position].node_name;
                    let minimal_token_width = self
                        .layout_info
                        .min_token_width
                        .get(self.token[token_position].start)
                        .copied();
                    let token_start = self.token[token_position].start;
                    let response = TokenEditor::with_min_width(
                        &self.token[token_position],
                        self.selected_nodes.contains(token_node_name),
                        minimal_token_width,
                    )
                    .ui(ui);
                    if response.clicked() {
                        let shift_pressed = ui.ctx().input(|i| i.modifiers.shift_only());
                        if shift_pressed {
                            self.select_range(token_position);
                        } else if ui.ctx().input(|i| i.modifiers.command_only()) {
                            if self.selected_nodes.contains(token_node_name) {
                                // Unselect
                                self.selected_nodes.remove(token_node_name);
                            } else {
                                // Allow selection of multiple items
                                self.selected_nodes.insert(token_node_name.clone());
                            }
                        } else {
                            // Select only one node
                            self.selected_nodes.clear();
                            self.selected_nodes.insert(token_node_name.clone());
                        }
                    }
                    let token_rect = response.rect;
                    current_span_offset = current_span_offset.max(token_rect.bottom());
                    token_offset_to_rect[token_start] = Some(token_rect);

                    if !self.layout_info.valid {
                        let offset_range = token_rect.x_range();
                        self.layout_info.token_offset_start[token_start] = offset_range.min;
                        self.layout_info.token_offset_end[token_start] = offset_range.max;
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
                self.show_segmentation_layers(ui, &token_offset_to_rect, current_span_offset)
            });

            // Add additional space for the scrollbar
            ui.add_space(10.0);

            if visible_range.start == 0.0 && !self.layout_info.min_token_width.is_empty() {
                self.layout_info.valid = true;
            }
            self.apply_pending_updates_for_editor();
        });

        self.layout_info.first_frame = false;
    }

    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn has_pending_updates(&self) -> bool {
        !self.pending_actions.is_empty()
    }

    fn apply_pending_updates_for_editor(&mut self) {
        if !self.has_pending_updates() {
            return;
        }
        let graph = self.graph.clone();
        let pending_actions = std::mem::take(&mut self.pending_actions);
        let parent_name = self.parent_name.clone();
        self.jobs.add(
            "Applying editor action",
            move |_job| {
                let mut graph_updates = GraphUpdate::new();
                let graph = graph.read();

                let mut state_updates = Vec::new();
                for action in pending_actions {
                    let editor_state_update =
                        action.apply(&graph, &parent_name, &mut graph_updates)?;
                    state_updates.push(editor_state_update);
                }

                Ok((graph_updates, state_updates))
            },
            |(graph_updates, state_updates), app| {
                app.project.add_changeset(graph_updates);
                if let Some(editor) = app.current_editor.get_mut() {
                    let downcasted = editor.any_mut().downcast_mut::<DocumentEditor>();
                    if let Some(editor) = downcasted {
                        for u in state_updates {
                            u(editor);
                        }
                    }
                }
            },
        );
    }

    fn get_selected_corpus_node(&self) -> Option<NodeID> {
        None
    }

    fn consume_shortcuts(&mut self, ctx: &egui::Context) {
        if !self.selected_nodes.is_empty() && self.currently_edited_node.is_none() {
            if ctx.input_mut(|i| i.consume_shortcut(&DELETE_SHORTCUT)) {
                self.delete_selected_nodes();
            } else {
                for layer_idx in 1..self.segmentations.len() {
                    if let Some(key) = Key::from_name(&layer_idx.to_string()) {
                        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
                            self.add_segmentation_for_selection(layer_idx);
                        }
                    }
                }
            }
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

impl EditorActions {
    fn apply(
        self,
        graph: &AnnotationGraph,
        parent_name: &str,
        updates: &mut GraphUpdate,
    ) -> anyhow::Result<StateUpdateFn> {
        let state_update: StateUpdateFn = match self {
            EditorActions::ModifySegmentationValue {
                node_name,
                new_value,
            } => {
                updates.add_event(UpdateEvent::DeleteNodeLabel {
                    node_name: node_name.to_string(),
                    anno_ns: TOKEN_KEY.ns.clone().into(),
                    anno_name: TOKEN_KEY.name.clone().into(),
                })?;
                updates.add_event(UpdateEvent::AddNodeLabel {
                    node_name: node_name.to_string(),
                    anno_ns: TOKEN_KEY.ns.clone().into(),
                    anno_name: TOKEN_KEY.name.clone().into(),
                    anno_value: new_value.to_string(),
                })?;
                Box::new(|_| {})
            }
            EditorActions::AddSegmentationSpan {
                segmentation,
                selected_token: selected_nodes,
            } => apply_add_segmentation(graph, parent_name, updates, segmentation, selected_nodes)?,
            EditorActions::DeleteNode { node_name } => {
                let node_id = graph
                    .get_node_annos()
                    .get_node_id_from_name(&node_name)?
                    .context("Missing node ID")?;
                updates.add_event(UpdateEvent::DeleteNode {
                    node_name: node_name.to_string(),
                })?;
                // Bridge any ordering edges that connect to this node to the remaining ones before and after
                for c in graph.get_all_components(Some(AnnotationComponentType::Ordering), None) {
                    if let Some(gs) = graph.get_graphstorage_as_ref(&c) {
                        let mut ingoing = gs.get_ingoing_edges(node_id);
                        let mut outgoing = gs.get_outgoing_edges(node_id);
                        if let (Some(ingoing), Some(outgoing)) = (ingoing.next(), outgoing.next()) {
                            let ingoing = graph
                                .get_node_annos()
                                .get_value_for_item(&ingoing?, &NODE_NAME_KEY)?
                                .context("Missing node name")?;
                            let outgoing = graph
                                .get_node_annos()
                                .get_value_for_item(&outgoing?, &NODE_NAME_KEY)?
                                .context("Missing node name")?;
                            updates.add_event(UpdateEvent::DeleteEdge {
                                source_node: ingoing.to_string(),
                                target_node: outgoing.to_string(),
                                layer: c.layer.to_string(),
                                component_type: c.get_type().to_string(),
                                component_name: c.name.to_string(),
                            })?;
                        }
                    }
                }
                Box::new(|_| {})
            }
        };
        Ok(state_update)
    }
}

fn apply_add_segmentation(
    graph: &AnnotationGraph,
    parent_name: &str,
    updates: &mut GraphUpdate,
    segmentation: String,
    selected_token: HashSet<String>,
) -> anyhow::Result<StateUpdateFn> {
    let new_node_name = format!(
        "{}#{}",
        &parent_name,
        graph
            .get_node_annos()
            .get_largest_item()?
            .map(|id| id + 1)
            .unwrap_or_default()
    );
    let tok_helper = TokenHelper::new(graph)?;
    let mut sorted_covered_token = Vec::new();
    for node_name in selected_token {
        let n = graph
            .get_node_annos()
            .get_node_id_from_name(&node_name)?
            .context("Missing node id")?;
        sorted_covered_token.push((n, node_name));
    }
    if let Some(gs) = tok_helper.get_ordering_gs(None) {
        sorted_covered_token.sort_by(|a, b| {
            if a == b {
                Ordering::Equal
            } else if let Ok(connected) = gs.is_connected(a.0, b.0, 1, std::ops::Bound::Unbounded) {
                if connected {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            } else {
                Ordering::Less
            }
        });
    }

    updates.add_event(UpdateEvent::AddNode {
        node_name: new_node_name.clone(),
        node_type: "node".to_string(),
    })?;
    updates.add_event(UpdateEvent::AddEdge {
        source_node: new_node_name.clone(),
        target_node: parent_name.to_string(),
        layer: ANNIS_NS.to_string(),
        component_type: AnnotationComponentType::PartOf.to_string(),
        component_name: "".to_string(),
    })?;
    updates.add_event(UpdateEvent::AddNodeLabel {
        node_name: new_node_name.clone(),
        anno_ns: TOKEN_KEY.ns.to_string(),
        anno_name: TOKEN_KEY.name.to_string(),
        anno_value: String::default(),
    })?;
    updates.add_event(UpdateEvent::AddNodeLabel {
        node_name: new_node_name.clone(),
        anno_ns: ANNIS_NS.to_string(),
        anno_name: segmentation.clone(),
        anno_value: String::default(),
    })?;

    for target_node in &sorted_covered_token {
        updates.add_event(UpdateEvent::AddEdge {
            source_node: new_node_name.clone(),
            target_node: target_node.1.clone(),
            layer: "".to_string(),
            component_type: AnnotationComponentType::Coverage.to_string(),
            component_name: "".to_string(),
        })?;
    }
    let first_covered = sorted_covered_token.first().cloned();
    let last_covered = sorted_covered_token.last().cloned();

    // Find the segmentations node before and after the selection to add the Ordering edges
    let matching_ordering_components =
        graph.get_all_components(Some(AnnotationComponentType::Ordering), Some(&segmentation));
    if let Some(ordering_component) = matching_ordering_components.first() {
        if let Some(first_covered) = &first_covered {
            if let Some(token_before) =
                tok_helper.get_token_before(first_covered.0, Some(&segmentation))?
            {
                let token_before = graph
                    .get_node_annos()
                    .get_value_for_item(&token_before, &NODE_NAME_KEY)?
                    .context("Missing node name")?;

                updates.add_event(UpdateEvent::AddEdge {
                    source_node: token_before.to_string(),
                    target_node: new_node_name.clone(),
                    layer: ordering_component.layer.to_string(),
                    component_type: ordering_component.get_type().to_string(),
                    component_name: ordering_component.name.to_string(),
                })?;
            }
        }
        if let Some(last_covered) = &last_covered {
            if let Some(token_after) =
                tok_helper.get_token_after(last_covered.0, Some(&segmentation))?
            {
                let token_after = graph
                    .get_node_annos()
                    .get_value_for_item(&token_after, &NODE_NAME_KEY)?
                    .context("Missing node name")?;

                updates.add_event(UpdateEvent::AddEdge {
                    source_node: new_node_name.clone(),
                    target_node: token_after.to_string(),
                    layer: ordering_component.layer.to_string(),
                    component_type: ordering_component.get_type().to_string(),
                    component_name: ordering_component.name.to_string(),
                })?;
            }
        }
    }

    let state_updater = Box::new(move |editor: &mut DocumentEditor| {
        let base_token_length = editor
            .segmentations
            .get("")
            .map(|token| token.len())
            .unwrap_or(0);
        if let (Some(seg_token), Some(first_covered), Some(last_covered)) = (
            editor.segmentations.get_mut(&segmentation),
            first_covered,
            last_covered,
        ) {
            // Insert the newly generated segmentation token at the approbiate position
            let first_covered_idx = editor
                .token_index_by_name
                .get(&first_covered.1)
                .copied()
                .unwrap_or(0);

            let last_covered_idx = editor
                .token_index_by_name
                .get(&last_covered.1)
                .copied()
                .unwrap_or(base_token_length);
            let mut new_token_labels = BTreeMap::new();
            new_token_labels.insert(TOKEN_KEY.as_ref().clone(), String::default());
            new_token_labels.insert(
                AnnoKey {
                    name: segmentation.into(),
                    ns: ANNIS_NS.into(),
                },
                String::default(),
            );
            let new_token = Token {
                node_name: new_node_name.clone(),
                start: first_covered_idx,
                end: last_covered_idx,
                labels: new_token_labels,
            };
            match seg_token.binary_search_by(|probe| probe.end.cmp(&first_covered_idx)) {
                Ok(idx) => seg_token.insert(idx + 1, new_token),
                Err(idx) => seg_token.insert(idx, new_token),
            }
        }
    });
    Ok(state_updater)
}
