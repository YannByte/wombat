//! Body-free trajectory controls accompany block locators. Inspection can reduce
//! the original source order without decoding unrelated historical event blocks.
use super::*;
use crate::session_events::{Event, ItemKind, LifecycleKind, MessageOrigin, Payload, Position};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub(crate) enum OrderKind {
    Gap,
    Compaction,
    Context {
        model: Option<String>,
    },
    Measurement {
        id: String,
        request_scoped: bool,
        model: Option<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OrderPoint {
    pub position: Position,
    pub gap: bool,
    pub value: OrderKind,
}
impl OrderPoint {
    pub(crate) fn of(event: &Event) -> Option<Self> {
        let gap = !event.gaps().is_empty();
        let value = match event.payload() {
            Payload::Measurement { value, .. } => OrderKind::Measurement {
                id: value.id.clone(),
                request_scoped: value.request_scoped,
                model: value.model.raw.as_deref().map(str::to_owned),
            },
            Payload::ContextWindow { model, .. } => OrderKind::Context {
                model: model.clone(),
            },
            Payload::Lifecycle {
                lifecycle: LifecycleKind::Compaction,
                ..
            }
            | Payload::Message {
                origin: MessageOrigin::Compaction,
                ..
            }
            | Payload::Item {
                item_kind: ItemKind::Compaction,
                ..
            } => OrderKind::Compaction,
            _ if gap => OrderKind::Gap,
            _ => return None,
        };
        Some(Self {
            position: event.position().clone(),
            gap,
            value,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectionIndex {
    pub time: TimeRange,
    pub has_gaps: bool,
    pub(crate) order: Vec<OrderPoint>,
}
impl InspectionIndex {
    pub(crate) fn build(events: &[Arc<Event>]) -> Self {
        Self {
            time: TimeRange::build(events.iter().map(|e| e.time().timestamp.as_deref())),
            has_gaps: events.iter().any(|e| !e.gaps().is_empty()),
            order: events.iter().filter_map(|e| OrderPoint::of(e)).collect(),
        }
    }
    pub(super) fn validate(&self, chunk: &EventChunk) -> Result<()> {
        self.time.validate(chunk.count)?;
        if self.order.len() > chunk.count
            || self.order.iter().any(|p| {
                p.position.event_id().is_err()
                    || super::events::position_order(&p.position, &chunk.first).is_lt()
                    || super::events::position_order(&p.position, &chunk.last).is_gt()
                    || p.gap && !self.has_gaps
                    || matches!(&p.value, OrderKind::Measurement { id, .. } if id.is_empty())
            })
            || self
                .order
                .windows(2)
                .any(|p| !super::events::position_order(&p[0].position, &p[1].position).is_lt())
        {
            return Err(corrupt("inspection block index invalid"));
        }
        Ok(())
    }
}

impl Snapshot {
    /// Materialize only a planned block. The caller meters the complete touched
    /// encoding and fact count before decoding; generic pages keep their own bounds.
    pub(crate) fn inspection_event_chunk(
        &self,
        partition: &EventPartition,
        chunk: &EventChunk,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<Arc<Event>>> {
        self.event_range(
            partition,
            chunk.offset,
            chunk.count,
            EventReadBudget {
                max_facts: chunk.count,
                max_bytes: chunk.bytes,
            },
            cancelled,
        )
    }
}
