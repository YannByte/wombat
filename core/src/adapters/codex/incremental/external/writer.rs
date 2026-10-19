//! Bounded direct-to-block publication; membership retains only scalar keys.
use super::*;
pub(super) struct FactWriter<'a> {
    db: &'a Connection,
    scope: String,
    field: &'static str,
    batch: Vec<(String, Box<RawValue>)>,
    bytes: usize,
}
impl<'a> FactWriter<'a> {
    pub(super) fn new(db: &'a Connection, source: &str, field: &'static str) -> Self {
        Self {
            db,
            scope: format!("projection:{source}"),
            field,
            batch: Vec::new(),
            bytes: 0,
        }
    }
    pub(super) fn push(&mut self, id: &str, thread: &str, value: &impl Serialize) -> Result<()> {
        output_key(self.db, self.field, id, thread)?;
        self.push_selected(id, value)
    }
    pub(super) fn push_selected(&mut self, id: &str, value: &impl Serialize) -> Result<()> {
        let payload = serde_json::value::to_raw_value(value)?;
        let bytes = id.len() + payload.get().len();
        if !self.batch.is_empty() && (self.batch.len() >= 128 || self.bytes + bytes > 256 * 1024) {
            self.flush()?;
        }
        self.batch.push((id.to_owned(), payload));
        self.bytes += bytes;
        Ok(())
    }
    pub(super) fn flush(&mut self) -> Result<()> {
        if !self.batch.is_empty() {
            crate::live_index::put_packed(self.db, &self.scope, self.field, self.batch.drain(..))?;
            self.bytes = 0;
        }
        Ok(())
    }
}
pub(super) fn output_key(db: &Connection, field: &str, id: &str, thread: &str) -> Result<()> {
    db.prepare_cached("INSERT INTO codex_output VALUES(?1,?2,?3,'')")?
        .execute(params![field, id, thread])?;
    Ok(())
}
