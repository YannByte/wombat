//! Exact thread selection over bounded source-local observation blocks.
use super::*;

pub(crate) struct EventReader {
    scope: String,
}
pub(crate) fn prepare_event_reader(_db: &Connection, scope: &str) -> Result<EventReader> {
    Ok(EventReader {
        scope: scope.to_owned(),
    })
}
impl EventReader {
    pub(crate) fn each(
        &self,
        db: &Connection,
        threads: &[&str],
        unassigned: bool,
        consume: impl FnMut(&str, &str) -> Result<()>,
    ) -> Result<()> {
        super::event_blocks::each(db, &self.scope, Some((threads, unassigned)), consume)?;
        Ok(())
    }
}
