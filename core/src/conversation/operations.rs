//! Exact operation navigation reuses canonical aliases; no text or time matching.
use super::*;
pub(super) struct OperationSelection(Arc<crate::adapters::contract::Operation>);
impl OperationSelection {
    pub(super) fn resolve(
        snapshot: &Snapshot,
        selection: &Selection,
        projects: &BTreeMap<&str, &crate::usage_store::ThreadEntry>,
    ) -> Result<Option<Self>> {
        let Some(id) = &selection.operation_id else {
            return Ok(None);
        };
        let thread = &selection.threads[0];
        if !projects.contains_key(thread.as_str()) {
            return Err(error("NOT_FOUND"));
        }
        let operation = snapshot
            .operations_for_turn(thread, &selection.turns[0])?
            .into_iter()
            .find(|operation| &operation.id == id)
            .ok_or_else(|| error("NOT_FOUND"))?;
        Ok(Some(Self(operation)))
    }
    pub(super) fn matches(&self, event: &Event) -> bool {
        let Payload::Operation { value, .. } = event.payload() else {
            return false;
        };
        value.id == self.0.id
            || value
                .call_id
                .as_ref()
                .zip(self.0.call_id.as_ref())
                .is_some_and(|(a, b)| a == b)
            || value
                .item_id
                .as_ref()
                .zip(self.0.item_id.as_ref())
                .is_some_and(|(a, b)| a == b)
    }
    pub(super) fn id(&self) -> String {
        self.0.id.to_string()
    }
}
