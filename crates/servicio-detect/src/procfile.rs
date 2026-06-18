use crate::{Detector, SuggestionDraft};
use std::path::Path;
pub struct Procfile;

impl Detector for Procfile {
    fn name(&self) -> &str { "procfile" }
    fn detect(&self, _root: &Path) -> Vec<SuggestionDraft> { vec![] }
}
