use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SearchArtifactDocument {
    pub(crate) schema_version: String,
    pub(crate) model: SearchModelHeader,
    pub(crate) graph_artifact_hash: String,
    pub(crate) embeddings: Vec<SearchEmbedding>,
}

impl SearchArtifactDocument {
    pub(crate) fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SearchModelHeader {
    pub(crate) id: String,
    pub(crate) provider: String,
    pub(crate) dim: usize,
}

/// V1.7.2 (ADR-0040): the `adoc.search.v1` entry discriminator. A prose
/// entry's `content_hash` is derived from its Embedding Composition, not
/// from a graph node hash, and its cache reuse is keyed by that hash rather
/// than by its order-derived block id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchEntryKind {
    KnowledgeObject,
    Prose,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SearchEmbedding {
    pub(crate) id: String,
    pub(crate) entry_kind: SearchEntryKind,
    pub(crate) content_hash: String,
    pub(crate) vector: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_artifact_serializes_with_v1_7_2_shape() {
        let artifact = SearchArtifactDocument {
            schema_version: "adoc.search.v1".to_string(),
            model: SearchModelHeader {
                id: "bge-small-en-v1.5".to_string(),
                provider: "fastembed".to_string(),
                dim: 384,
            },
            graph_artifact_hash: "sha256:graph".to_string(),
            embeddings: vec![
                SearchEmbedding {
                    id: "billing.credits".to_string(),
                    entry_kind: SearchEntryKind::KnowledgeObject,
                    content_hash: "sha256:content".to_string(),
                    vector: vec![0.25, -0.5],
                },
                SearchEmbedding {
                    id: "guides.page#block-0002".to_string(),
                    entry_kind: SearchEntryKind::Prose,
                    content_hash: "sha256:prose".to_string(),
                    vector: vec![0.5, 0.25],
                },
            ],
        };

        let value = serde_json::to_value(&artifact).expect("search artifact serializes");

        assert_eq!(value["schema_version"], "adoc.search.v1");
        assert_eq!(value["model"]["id"], "bge-small-en-v1.5");
        assert_eq!(value["model"]["provider"], "fastembed");
        assert_eq!(value["model"]["dim"], 384);
        assert_eq!(value["graph_artifact_hash"], "sha256:graph");
        assert_eq!(value["embeddings"][0]["id"], "billing.credits");
        assert_eq!(value["embeddings"][0]["entry_kind"], "knowledge_object");
        assert_eq!(value["embeddings"][0]["content_hash"], "sha256:content");
        assert_eq!(
            value["embeddings"][0]["vector"],
            serde_json::json!([0.25, -0.5])
        );
        assert_eq!(value["embeddings"][1]["id"], "guides.page#block-0002");
        assert_eq!(value["embeddings"][1]["entry_kind"], "prose");
    }
}
