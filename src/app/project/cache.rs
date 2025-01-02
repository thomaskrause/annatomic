use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use egui::mutex::RwLock;
use graphannis::AnnotationGraph;

struct InnerCorpusCache {
    name: String,
    location: PathBuf,
    graph: Arc<RwLock<AnnotationGraph>>,
}
#[derive(Clone, Default)]
pub(crate) struct CorpusCache {
    inner: Arc<RwLock<Option<InnerCorpusCache>>>,
}

impl CorpusCache {
    pub(crate) fn get(
        &self,
        corpus_name: &str,
        corpus_location: &Path,
    ) -> Result<Option<Arc<RwLock<AnnotationGraph>>>> {
        {
            let mut inner = self.inner.write();

            // Check if a cached version exist
            if let Some(existing) = inner.as_mut() {
                if existing.name == corpus_name && existing.location == corpus_location {
                    return Ok(Some(existing.graph.clone()));
                }
            }
        }

        // Load and return the graph
        self.load_from_disk(corpus_name, corpus_location)
    }

    pub(crate) fn load_from_disk(
        &self,
        corpus_name: &str,
        corpus_location: &Path,
    ) -> Result<Option<Arc<RwLock<AnnotationGraph>>>> {
        let mut inner = self.inner.write();

        // Load and return the graph
        let mut graph = AnnotationGraph::new(false)?;
        graph.import(corpus_location)?;

        let graph = Arc::new(RwLock::new(graph));

        *inner = Some(InnerCorpusCache {
            graph: graph.clone(),
            location: corpus_location.to_path_buf(),
            name: corpus_name.to_string(),
        });
        Ok(Some(graph))
    }
}
