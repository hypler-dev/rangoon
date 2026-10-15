use rangoon_domain::AnalysisReport;
use std::sync::Mutex;

struct Current {
    generation: Option<u64>,
    report: Option<AnalysisReport>,
}
impl Default for Current {
    fn default() -> Self {
        Self {
            generation: Some(0),
            report: None,
        }
    }
}
#[derive(Default)]
pub struct Session(Mutex<Current>);
impl Session {
    pub fn generation(&self) -> Option<u64> {
        self.0.lock().ok().and_then(|s| s.generation)
    }
    pub fn accept(&self, generation: u64, report: &AnalysisReport) -> bool {
        let Ok(mut current) = self.0.lock() else {
            return false;
        };
        if current.generation != Some(generation) {
            return false;
        }
        current.report = Some(report.clone());
        true
    }
    pub fn clear(&self) -> bool {
        let (mut current, healthy) = match self.0.lock() {
            Ok(current) => (current, true),
            Err(error) => (error.into_inner(), false),
        };
        current.generation = current.generation.and_then(|value| value.checked_add(1));
        current.report = None;
        healthy && current.generation.is_some()
    }
    pub fn snapshot(&self, source_id: &str) -> Option<AnalysisReport> {
        self.0
            .lock()
            .ok()?
            .report
            .as_ref()
            .filter(|r| r.source.id == source_id)
            .cloned()
    }
    #[cfg(test)]
    pub(super) fn seed_generation(&self, value: u64) {
        let mut current = self.0.lock().unwrap();
        current.generation = Some(value);
        current.report = None;
    }
    #[cfg(test)]
    pub(super) fn poison_for_test(&self) {
        let _ = std::panic::catch_unwind(|| {
            let mut current = self.0.lock().unwrap();
            current.report = None;
            panic!("synthetic source session poison");
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clear_rejects_late_picker_or_open_result_and_stale_saves() {
        let session = Session::default();
        let report = rangoon_import::analyze("AGENTS.md", b"# Sample\n").unwrap();
        let generation = session.generation().unwrap();
        assert!(session.accept(generation, &report));
        assert!(session.snapshot(&report.source.id).is_some());
        assert!(session.snapshot("source:wrong").is_none());
        assert!(session.clear());
        assert!(!session.accept(generation, &report));
        assert!(session.snapshot(&report.source.id).is_none());
        assert!(session.accept(session.generation().unwrap(), &report));
        assert!(session.snapshot(&report.source.id).is_some());
    }
    #[test]
    fn replacement_rejects_old_source_id_but_save_copy_is_stable() {
        let session = Session::default();
        let first = rangoon_import::analyze("a.md", b"first").unwrap();
        let second = rangoon_import::analyze("a.md", b"second").unwrap();
        let generation = session.generation().unwrap();
        session.accept(generation, &first);
        let captured = session.snapshot(&first.source.id).unwrap();
        session.accept(generation, &second);
        assert!(session.snapshot(&first.source.id).is_none());
        session.clear();
        assert_eq!(captured, first);
    }
}
