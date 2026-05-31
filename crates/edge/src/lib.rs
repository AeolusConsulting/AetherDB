mod error;
mod search;
mod store;
mod sync;
#[cfg(feature = "wasm")]
mod wasm;

pub use error::EdgeError;
pub use search::SearchResult;
pub use store::EdgeStore;
pub use sync::{SyncDocument, SyncEmbedding, SyncPayload, SyncStats};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_search_roundtrip() {
        let mut store = EdgeStore::new(3);
        let id = store
            .insert_document_with_embedding(
                "hello world".into(),
                serde_json::json!({"source": "test"}),
                vec![1.0, 0.0, 0.0],
            )
            .unwrap();

        let results = store.search_similar(&[0.9, 0.1, 0.0], 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document_id, id);
        assert!(results[0].score > 0.9);
    }

    #[test]
    fn multiple_documents_ranked_by_similarity() {
        let mut store = EdgeStore::new(3);
        let id_a = store
            .insert_document_with_embedding(
                "doc a".into(),
                serde_json::json!({}),
                vec![1.0, 0.0, 0.0],
            )
            .unwrap();
        let _id_b = store
            .insert_document_with_embedding(
                "doc b".into(),
                serde_json::json!({}),
                vec![0.0, 1.0, 0.0],
            )
            .unwrap();

        let results = store.search_similar(&[0.9, 0.1, 0.0], 5).unwrap();
        assert_eq!(results[0].document_id, id_a);
    }

    #[test]
    fn dimension_mismatch_rejected() {
        let mut store = EdgeStore::new(3);
        let result = store.insert_document_with_embedding(
            "bad".into(),
            serde_json::json!({}),
            vec![1.0, 0.0],
        );
        assert!(matches!(result, Err(EdgeError::DimensionMismatch { .. })));
    }

    #[test]
    fn empty_content_rejected() {
        let mut store = EdgeStore::new(3);
        let result = store.insert_document("".into(), serde_json::json!({}));
        assert!(matches!(result, Err(EdgeError::Validation(_))));
    }

    #[test]
    fn delete_removes_document_and_embedding() {
        let mut store = EdgeStore::new(3);
        let id = store
            .insert_document_with_embedding("x".into(), serde_json::json!({}), vec![1.0, 0.0, 0.0])
            .unwrap();
        assert_eq!(store.document_count(), 1);
        assert_eq!(store.embedding_count(), 1);

        assert!(store.delete_document(&id));
        assert_eq!(store.document_count(), 0);
        assert_eq!(store.embedding_count(), 0);
    }

    #[test]
    fn export_import_sync_roundtrip() {
        let mut store = EdgeStore::new(3);
        store
            .insert_document_with_embedding(
                "doc one".into(),
                serde_json::json!({}),
                vec![1.0, 0.0, 0.0],
            )
            .unwrap();

        let payload = store.export_for_sync().unwrap();
        let json = serde_json::to_string(&payload).unwrap();

        let mut store2 = EdgeStore::new(3);
        let parsed: SyncPayload = serde_json::from_str(&json).unwrap();
        let stats = store2.import_from_sync(&parsed).unwrap();
        assert_eq!(stats.documents_imported, 1);
        assert_eq!(stats.embeddings_imported, 1);
        assert_eq!(store2.document_count(), 1);
    }

    #[test]
    fn import_skips_existing_documents() {
        let mut store = EdgeStore::new(3);
        store
            .insert_document_with_embedding(
                "existing".into(),
                serde_json::json!({}),
                vec![1.0, 0.0, 0.0],
            )
            .unwrap();

        let payload = store.export_for_sync().unwrap();
        let stats = store.import_from_sync(&payload).unwrap();
        assert_eq!(stats.documents_skipped, 1);
        assert_eq!(stats.documents_imported, 0);
    }

    #[test]
    fn search_empty_store_returns_empty() {
        let store = EdgeStore::new(3);
        let results = store.search_similar(&[1.0, 0.0, 0.0], 5).unwrap();
        assert!(results.is_empty());
    }
}
