use crate::{Detector, SuggestionDraft};
use std::path::Path;
pub struct Python;

impl Detector for Python {
    fn name(&self) -> &str { "python" }
    fn detect(&self, _root: &Path) -> Vec<SuggestionDraft> { vec![] }
}
