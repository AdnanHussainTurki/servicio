use crate::{Detector, SuggestionDraft};
use std::path::Path;
pub struct Node;

impl Detector for Node {
    fn name(&self) -> &str { "node" }
    fn detect(&self, _root: &Path) -> Vec<SuggestionDraft> { vec![] }
}
