/// Errors that may surface during the lowering / attribute pass.
///
/// Empty for now — lowering the current grammar coverage is total.
/// Real variants will arrive once lowering does reference resolution
/// or type validation that can actually fail.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SemanticError {}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {}
    }
}

impl std::error::Error for SemanticError {}
