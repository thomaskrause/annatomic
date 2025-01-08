use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use egui::mutex::RwLock;
use graphannis::AnnotationGraph;

struct InnerCorpusCache {
    location: PathBuf,
    graph: Arc<RwLock<AnnotationGraph>>,
}
#[derive(Clone, Default)]
pub(crate) struct CorpusCache {
    inner: Arc<RwLock<Option<InnerCorpusCache>>>,
}

impl CorpusCache {
    pub(crate) fn get(&self, location: &Path) -> Result<Arc<RwLock<AnnotationGraph>>> {
        {
            let mut inner = self.inner.write();

            // Check if a cached version exist
            if let Some(existing) = inner.as_mut() {
                if existing.location == location {
                    return Ok(existing.graph.clone());
                } else {
                    // Drop the annotation graph in background thread, so we can return faster
                    let old_graph = inner.take();
                    std::thread::spawn(move || std::mem::drop(old_graph));
                }
            }
        }

        // Load and return the graph
        self.load_from_disk(location)
    }

    pub(crate) fn load_from_disk(
        &self,
        corpus_location: &Path,
    ) -> Result<Arc<RwLock<AnnotationGraph>>> {
        let mut inner = self.inner.write();

        // Load and return the graph
        let mut graph = AnnotationGraph::new(false)?;
        graph.import(corpus_location)?;

        let graph = Arc::new(RwLock::new(graph));

        *inner = Some(InnerCorpusCache {
            graph: graph.clone(),
            location: corpus_location.to_path_buf(),
        });
        Ok(graph)
    }
}
