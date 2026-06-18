use crate::{Detector, SuggestionDraft};
use std::path::Path;
pub struct Crontab;

impl Detector for Crontab {
    fn name(&self) -> &str { "crontab" }
    fn detect(&self, _root: &Path) -> Vec<SuggestionDraft> { vec![] }
}
