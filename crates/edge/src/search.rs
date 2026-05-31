use aetherdb_domain::DocumentId;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub document_id: DocumentId,
    pub score: f32,
    pub distance: f32,
}

pub(crate) fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denom = norm_a * norm_b;
    if denom == 0.0 {
        1.0
    } else {
        1.0 - (dot / denom)
    }
}

pub(crate) fn brute_force_search(
    query: &[f32],
    embeddings: &[(DocumentId, &[f32])],
    top_k: usize,
) -> Vec<SearchResult> {
    let mut scored: Vec<(DocumentId, f32)> = embeddings
        .iter()
        .map(|(id, vec)| (*id, cosine_distance(query, vec)))
        .collect();

    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);

    scored
        .into_iter()
        .map(|(document_id, distance)| {
            let score = (1.0_f32 - distance).clamp(0.0, 1.0);
            SearchResult {
                document_id,
                score,
                distance,
            }
        })
        .collect()
}
