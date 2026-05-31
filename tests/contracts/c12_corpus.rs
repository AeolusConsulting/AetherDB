// Contract C12 — benchmark corpus.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

#[test]
#[ignore = "run after `make bench-seed` (downloads ~100MB)"]
fn corpus_file_exists_and_is_jsonl() {
    let path = workspace_root().join("tests/corpus/msmarco_10k.json");
    assert!(path.exists(), "corpus file missing at {}", path.display());
    let content = std::fs::read_to_string(&path).expect("read corpus");
    let mut count = 0;
    for line in content.lines() {
        let v: serde_json::Value = serde_json::from_str(line).expect("each line is valid JSON");
        assert!(v.get("id").is_some(), "each line must have an 'id' field");
        assert!(v.get("text").is_some(), "each line must have a 'text' field");
        count += 1;
    }
    assert_eq!(count, 10_000, "corpus should have 10k lines, got {count}");
}

#[test]
#[ignore = "run after `make bench-seed`"]
fn corpus_matches_committed_checksum() {
    use sha2::{Digest, Sha256};
    let path = workspace_root().join("tests/corpus/msmarco_10k.json");
    let expected = std::fs::read_to_string(workspace_root().join("tests/corpus/msmarco_10k.sha256"))
        .expect("read sha256");
    let expected = expected.split_whitespace().next().expect("sha256 hex");
    let bytes = std::fs::read(&path).expect("read corpus");
    let actual = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(actual, expected, "corpus file does not match committed checksum");
}

#[test]
fn benchmarks_md_has_baseline_row() {
    let path = workspace_root().join("docs/BENCHMARKS.md");
    assert!(path.exists(), "BENCHMARKS.md missing");
    let content = std::fs::read_to_string(&path).expect("read");
    // Must have a row containing all four metrics.
    assert!(content.contains("p50") && content.contains("p95") &&
            content.contains("p99") && content.contains("recall@10"));
}
