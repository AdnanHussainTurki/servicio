use crate::{Detector, SuggestionDraft};
use std::path::Path;
pub struct Laravel;

impl Detector for Laravel {
    fn name(&self) -> &str { "laravel" }
    fn detect(&self, _root: &Path) -> Vec<SuggestionDraft> { vec![] }
}
