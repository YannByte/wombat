//! Bounded sharing of repetitive fact metadata. Identities and contents remain exact.
use super::contract::{Measurement, Operation};
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct FactStrings {
    values: crate::shared_text::SharedText,
    files: crate::shared_text::SharedText,
}

impl FactStrings {
    pub(crate) fn identity(&mut self, value: &mut Arc<str>) {
        self.values.share(value);
    }
    pub(crate) fn file(&mut self, value: &mut Arc<str>) {
        self.files.share(value);
    }

    fn evidence(&mut self, rows: &mut [super::contract::EvidenceRef]) {
        for row in rows {
            self.file(&mut row.file);
        }
    }
    fn optional(&mut self, value: &mut Option<Arc<str>>) {
        if let Some(value) = value {
            self.values.share(value);
        }
    }
    pub(crate) fn measurement(&mut self, row: &mut Measurement) {
        for value in [
            &mut row.agent_kind,
            &mut row.source_instance_id,
            &mut row.grain,
            &mut row.time_precision,
        ] {
            self.values.share(value);
        }
        for value in [&mut row.thread_id, &mut row.turn_id].into_iter().flatten() {
            self.identity(value);
        }
        self.evidence(&mut row.evidence);
        for value in [
            &mut row.model.raw,
            &mut row.model.provider,
            &mut row.model.api_provider,
            &mut row.model.pricing_model,
            &mut row.reasoning_effort,
        ] {
            self.optional(value);
        }
    }
    pub(crate) fn operation(&mut self, row: &mut Operation) {
        for value in [
            &mut row.kind,
            &mut row.name,
            &mut row.time_precision,
            &mut row.status,
        ] {
            self.values.share(value);
        }
        self.identity(&mut row.thread_id);
        if let Some(turn) = &mut row.turn_id {
            self.identity(turn);
        }
        self.evidence(&mut row.evidence);
        for value in [&mut row.server, &mut row.tool] {
            self.optional(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_churn_preserves_text_and_cannot_exhaust_the_file_pool() {
        let mut pool = FactStrings::default();
        for n in 0..6000 {
            let expected = format!("synthetic-{n}");
            let mut value: Arc<str> = expected.as_str().into();
            pool.identity(&mut value);
            assert_eq!(value.as_ref(), expected);
        }
        let mut file: Arc<str> = "/synthetic/source.jsonl".into();
        let mut other: Arc<str> = "/synthetic/source.jsonl".into();
        pool.file(&mut file);
        pool.file(&mut other);
        assert!(
            Arc::ptr_eq(&file, &other),
            "identity churn cannot exhaust the file pool"
        );
    }

    #[test]
    fn sharing_preserves_serialized_facts_and_prior_revision() {
        let original = serde_json::json!({"id":"m","agentKind":"codex","sourceInstanceId":"s","threadId":"t","turnId":"u","grain":"response","timePrecision":"unknown","model":{"raw":"same","provider":"provider","apiProvider":"different","pricingModel":"same"},"tokens":{"total":100},"tokenUnavailableReasons":{"input":"missing","cacheRead":"missing","cacheCreate":"missing","output":"missing","reasoning":"missing","total":null,"rawInput":"missing"},"pricingContextConflict":false,"requestScoped":true,"sequence":1,"evidence":[]});
        let mut first: Measurement = serde_json::from_value(original).unwrap();
        let expected = serde_json::to_value(&first).unwrap();
        let mut second = first.clone();
        let mut pool = FactStrings::default();
        pool.measurement(&mut first);
        pool.measurement(&mut second);
        assert_eq!(serde_json::to_value(&first).unwrap(), expected);
        assert!(Arc::ptr_eq(
            first.model.raw.as_ref().unwrap(),
            second.model.pricing_model.as_ref().unwrap()
        ));
        let prior = Arc::new(first);
        let mut changed = Arc::clone(&prior);
        Arc::make_mut(&mut changed).model.provider = Some("new-provider".into());
        pool.measurement(Arc::make_mut(&mut changed));
        assert_eq!(serde_json::to_value(prior.as_ref()).unwrap(), expected);
        assert_eq!(changed.model.provider.as_deref(), Some("new-provider"));
        assert_eq!(changed.model.api_provider.as_deref(), Some("different"));
    }
}
