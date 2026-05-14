//! Attribute pass: lower the syntactic AST from `mtg-grammar` into a
//! normalized semantic IR.
//!
//! See `ARCHITECTURE.md` for the lowering contract.

mod error;
mod ir;
mod lower;

pub use error::SemanticError;
pub use ir::{CardEffect, ManaValue};
pub use lower::lower;
