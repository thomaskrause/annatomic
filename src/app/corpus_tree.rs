use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use graphannis::{
    graph::AnnoKey,
    model::{AnnotationComponent, AnnotationComponentType::PartOf},
    CorpusStorage,
};

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct CorpusTree {
    pub(crate) node_name: String,
    pub(crate) annos: BTreeMap<String, String>,
    pub(crate) children: Vec<CorpusTree>,
}

impl CorpusTree {
    pub fn create_from_graphstorage(
        cs: Arc<CorpusStorage>,
        corpus_name: &str,
    ) -> anyhow::Result<Self> {
        let mut corpus_graph = cs.corpus_graph(corpus_name)?;
        corpus_graph.ensure_loaded_all()?;

        let mut children = Vec::new();

        if let Some(partof) = corpus_graph.get_graphstorage(&AnnotationComponent::new(
            PartOf,
            "annis".into(),
            "".into(),
        )) {
            let mut documents = Vec::new();
            for n in partof.root_nodes() {
                let n = n?;
                let node_name = corpus_graph.get_node_annos().get_value_for_item(
                    &n,
                    &AnnoKey {
                        name: "node_name".into(),
                        ns: "annis".into(),
                    },
                )?;
                documents.push(node_name.unwrap_or(Cow::Borrowed("<UNKNOWN>")));
            }
            documents.sort();
            for d in documents {
                let child_doc = CorpusTree {
                    node_name: d.to_string(),
                    annos: BTreeMap::new(),
                    children: vec![],
                };

                children.push(child_doc);
            }
        }

        let root = CorpusTree {
            node_name: corpus_name.to_string(),
            annos: BTreeMap::new(),
            children,
        };

        Ok(root)
    }
}
