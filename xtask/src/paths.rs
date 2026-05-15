use std::path::PathBuf;

/// Workspace root, resolved from this crate's `CARGO_MANIFEST_DIR` so
/// invocation directory doesn't matter.
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest has a parent directory")
        .to_path_buf()
}

pub fn corpus_status_path() -> PathBuf {
    repo_root().join("corpus_status.json")
}

pub fn generated_tests_dir() -> PathBuf {
    repo_root().join("crates/mtg-grammar/tests/generated")
}

pub fn generated_tests_manifest() -> PathBuf {
    repo_root().join("crates/mtg-grammar/tests/generated.rs")
}

pub fn generated_pattern_tests_dir() -> PathBuf {
    repo_root().join("crates/mtg-grammar/tests/generated_patterns")
}

pub fn generated_pattern_tests_manifest() -> PathBuf {
    repo_root().join("crates/mtg-grammar/tests/generated_patterns.rs")
}

pub fn grammar_fix_log_root() -> PathBuf {
    repo_root().join(".grammar-fix")
}

pub fn grammar_pest_path() -> PathBuf {
    repo_root().join("crates/mtg-grammar/src/grammar.pest")
}

pub fn ast_rs_path() -> PathBuf {
    repo_root().join("crates/mtg-grammar/src/ast.rs")
}

pub fn lower_rs_path() -> PathBuf {
    repo_root().join("crates/mtg-semantic/src/lower.rs")
}
