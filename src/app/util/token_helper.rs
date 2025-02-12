use anyhow::{anyhow, Context, Result};
use graphannis::{graph::GraphStorage, model::AnnotationComponentType, AnnotationGraph};
use graphannis_core::{
    annostorage::NodeAnnotationStorage,
    dfs,
    graph::{storage::union::UnionEdgeContainer, ANNIS_NS},
    types::{AnnoKey, Component, NodeID},
};

use itertools::Itertools;
use lazy_static::lazy_static;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

#[derive(Clone)]
pub struct TokenHelper<'a> {
    node_annos: &'a dyn NodeAnnotationStorage,
    cov_edges: Vec<Arc<dyn GraphStorage>>,
    ordering_gs: BTreeMap<String, Arc<dyn GraphStorage>>,
    part_of_gs: Arc<dyn GraphStorage>,
}

lazy_static! {
    static ref COMPONENT_LEFT: Component<AnnotationComponentType> = {
        Component::new(
            AnnotationComponentType::LeftToken,
            ANNIS_NS.into(),
            "".into(),
        )
    };
    static ref COMPONENT_RIGHT: Component<AnnotationComponentType> = {
        Component::new(
            AnnotationComponentType::RightToken,
            ANNIS_NS.into(),
            "".into(),
        )
    };
    pub static ref TOKEN_KEY: Arc<AnnoKey> = Arc::from(AnnoKey {
        ns: ANNIS_NS.into(),
        name: "tok".into(),
    });
}

impl<'a> TokenHelper<'a> {
    pub fn new(graph: &'a AnnotationGraph) -> anyhow::Result<TokenHelper<'a>> {
        let cov_edges: Vec<Arc<dyn GraphStorage>> = graph
            .get_all_components(Some(AnnotationComponentType::Coverage), None)
            .into_iter()
            .filter_map(|c| graph.get_graphstorage(&c))
            .filter(|gs| {
                if let Some(stats) = gs.get_statistics() {
                    stats.nodes > 0
                } else {
                    true
                }
            })
            .collect();
        let mut ordering_gs = BTreeMap::new();

        for c in graph.get_all_components(Some(AnnotationComponentType::Ordering), None) {
            if let Some(gs) = graph.get_graphstorage(&c) {
                ordering_gs.insert(c.name.to_string(), gs);
            }
        }

        let part_of_component =
            Component::new(AnnotationComponentType::PartOf, ANNIS_NS.into(), "".into());
        let part_of_gs = graph
            .get_graphstorage(&part_of_component)
            .ok_or_else(|| anyhow!("Missing PartOf component"))?;

        Ok(TokenHelper {
            node_annos: graph.get_node_annos(),
            cov_edges,
            ordering_gs,
            part_of_gs,
        })
    }

    pub fn is_token(&self, id: NodeID) -> anyhow::Result<bool> {
        if self.node_annos.has_value_for_item(&id, &TOKEN_KEY)? {
            // check if there is no outgoing edge in any of the coverage components
            let has_outgoing = self.has_outgoing_coverage_edges(id)?;
            Ok(!has_outgoing)
        } else {
            Ok(false)
        }
    }

    pub fn has_outgoing_coverage_edges(&self, id: NodeID) -> anyhow::Result<bool> {
        for c in self.cov_edges.iter() {
            if c.has_outgoing_edges(id)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn get_ordered_token(
        &self,
        parent_name: &str,
        segmentation: Option<&str>,
    ) -> Result<Vec<NodeID>> {
        let parent_id = self.node_annos.get_node_id_from_name(parent_name)?;
        let segmentation = segmentation.unwrap_or("");
        let ordering_gs = &self
            .ordering_gs
            .get(segmentation)
            .ok_or_else(|| anyhow!("Missing ordering component for segmentation {segmentation}"))?;

        // Find all token roots
        let mut roots: HashSet<_> = HashSet::new();
        for n in ordering_gs.source_nodes() {
            let n = n?;
            if !ordering_gs.has_ingoing_edges(n)? {
                // Filter the roots by checking the parent node in the corpus structure
                if let Some(parent_id) = parent_id {
                    if self
                        .part_of_gs
                        .is_connected(n, parent_id, 1, std::ops::Bound::Unbounded)?
                    {
                        roots.insert(n);
                    }
                } else {
                    roots.insert(n);
                }
            }
        }

        // Follow the ordering edges from the roots to reconstruct the token in their correct order
        let mut result = Vec::default();
        for r in roots {
            let mut token = Some(r);
            while let Some(current_token) = token {
                result.push(current_token);
                // Get next token
                if let Some(next_token) = ordering_gs.get_outgoing_edges(current_token).next() {
                    let next_token = next_token?;
                    token = Some(next_token);
                } else {
                    token = None;
                }
            }
        }

        Ok(result)
    }

    #[cfg(test)]
    pub fn spanned_text(&self, token_ids: &[NodeID]) -> Result<String> {
        use graphannis_core::errors::GraphAnnisCoreError;
        use itertools::Itertools;

        let anno_values: std::result::Result<Vec<_>, GraphAnnisCoreError> = token_ids
            .iter()
            .map(|t| self.node_annos.get_value_for_item(t, &TOKEN_KEY))
            .collect();
        // TODO: support whitespace after/before annotations
        let anno_values = anno_values?.into_iter().flatten().collect_vec();
        let result = anno_values.join(" ");
        Ok(result)
    }

    /// Find all token covered by the given node and sort the result according to the token order.
    pub fn covered_token(&self, node_id: NodeID) -> Result<Vec<NodeID>> {
        let mut result = Vec::default();
        let coverage = UnionEdgeContainer::new(
            self.cov_edges
                .iter()
                .map(|gs| gs.as_edgecontainer())
                .collect_vec(),
        );
        let it = dfs::CycleSafeDFS::new(&coverage, node_id, 1, usize::MAX);
        for step in it {
            let step = step?;
            if self.is_token(step.node)? {
                result.push(step.node);
            }
        }

        // Sort token by their order
        self.sort_token(&mut result, None)?;
        Ok(result)
    }

    pub fn get_ordering_gs(&self, segmentation: Option<&str>) -> Option<Arc<dyn GraphStorage>> {
        self.ordering_gs
            .get(segmentation.unwrap_or_default())
            .cloned()
    }

    pub fn sort_token(&self, token_ids: &mut [NodeID], segmentation: Option<&str>) -> Result<()> {
        if let Some(gs) = self.ordering_gs.get(segmentation.unwrap_or_default()) {
            token_ids.sort_by(|a, b| {
                if a == b {
                    Ordering::Equal
                } else if let Ok(connected) = gs.is_connected(*a, *b, 1, std::ops::Bound::Unbounded)
                {
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
        Ok(())
    }

    /// Gets the token that comes before the given `node_id`. If a
    /// `segmentation` is given, this will not be a base token but a
    /// segmentation node that comes directly before the given node.
    pub fn get_token_before(
        &self,
        node_id: NodeID,
        segmentation: Option<&str>,
    ) -> Result<Option<NodeID>> {
        // Get all sorted covered token for this node
        let covered_token = if self.is_token(node_id)? {
            vec![node_id]
        } else {
            self.covered_token(node_id)?
        };

        // Find the token node before the left-most covered token
        if let Some(first_covered_token) = covered_token.first() {
            let gs_tok = self
                .ordering_gs
                .get("")
                .context("Missing base token graph storage component")?;
            if let Some(token_before) = gs_tok.get_ingoing_edges(*first_covered_token).next() {
                let token_before = token_before?;

                if let Some(segmentation) = segmentation {
                    // If a segmentation node is requested as result, find the one covering this token

                    let mut segmentation_nodes = Vec::new();
                    for gs_cov in &self.cov_edges {
                        for n in gs_cov.get_ingoing_edges(token_before) {
                            segmentation_nodes.push(n?);
                        }
                    }

                    self.sort_token(&mut segmentation_nodes, Some(segmentation))?;
                    Ok(segmentation_nodes.last().copied())
                } else {
                    Ok(Some(token_before))
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Gets the token that comes after the given `node_id`. If a `segmentation`
    /// is given, this will not be a base token but a segmentation node that
    /// comes directly after the given node.
    pub fn get_token_after(
        &self,
        node_id: NodeID,
        segmentation: Option<&str>,
    ) -> Result<Option<NodeID>> {
        // Get all sorted covered token for this node
        let covered_token = if self.is_token(node_id)? {
            vec![node_id]
        } else {
            self.covered_token(node_id)?
        };

        // Find the token node after the right-most covered token
        if let Some(last_covered_token) = covered_token.last() {
            let gs_tok = self
                .ordering_gs
                .get("")
                .context("Missing base token graph storage component")?;
            if let Some(token_after) = gs_tok.get_outgoing_edges(*last_covered_token).next() {
                let token_after = token_after?;

                if let Some(segmentation) = segmentation {
                    // If a segmentation node is requested as result, find the one covering this token
                    let mut segmentation_nodes = Vec::new();
                    for gs_cov in &self.cov_edges {
                        for n in gs_cov.get_ingoing_edges(token_after) {
                            segmentation_nodes.push(n?);
                        }
                    }
                    self.sort_token(&mut segmentation_nodes, Some(segmentation))?;
                    Ok(segmentation_nodes.first().copied())
                } else {
                    Ok(Some(token_after))
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests;
