//! Read measurements and canonical operations in one bounded, verified turn slice.
use super::timing_evidence::{Meter, TurnTarget};
use super::*;
use serde::{
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
    ser::SerializeSeq,
};
use sha2::{Digest, Sha256};
use std::{fmt, io};

pub(super) struct CanonicalTurnFacts {
    pub measurements: Vec<PricedMeasurement>,
    pub operations: Vec<Arc<Operation>>,
}

struct LiveRows<'a> {
    indices: &'a [usize],
    rows: &'a [Arc<PricedMeasurement>],
}
impl Serialize for LiveRows<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.indices.len()))?;
        for index in self.indices {
            seq.serialize_element(&self.rows[*index])?;
        }
        seq.end()
    }
}
#[derive(Serialize)]
struct LiveTurn<'a> {
    measurements: LiveRows<'a>,
    operations: &'a [Arc<Operation>],
}
struct Writer<'a, 'b>(&'a mut Meter<'b>);
impl io::Write for Writer<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.bytes(bytes.len() as u64).map_err(io::Error::other)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn failure(meter: &mut Meter<'_>, error: impl fmt::Display) -> anyhow::Error {
    match meter.failure {
        Some("CANCELLED") => operation_error("CANCELLED", "轮次证据读取已取消"),
        Some("RESOURCE_LIMIT") => operation_error("RESOURCE_LIMIT", "轮次证据读取超过资源预算"),
        _ => corrupt(format!("轮次计量分片无效：{error}")),
    }
}
fn exact(fact: &Measurement, target: TurnTarget<'_>) -> bool {
    fact.source_instance_id.as_ref() == target.source
        && fact.thread_id.as_deref() == Some(target.thread)
        && fact.turn_id.as_deref() == Some(target.turn)
}
struct Facts<'a, 'b> {
    meter: &'a mut Meter<'b>,
    target: TurnTarget<'a>,
}
impl<'de> DeserializeSeed<'de> for Facts<'_, '_> {
    type Value = Vec<PricedMeasurement>;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for Facts<'_, '_> {
    type Value = Vec<PricedMeasurement>;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded canonical measurements")
    }
    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut out = vec![];
        // Each seed charges before parsing or allocating the next fact.
        while let Some(row) = seq.next_element_seed(OneFact { meter: self.meter })? {
            if !exact(&row.fact, self.target) {
                return Err(serde::de::Error::custom("measurement attribution mismatch"));
            }
            out.push(row);
        }
        Ok(out)
    }
}
struct OneFact<'a, 'b> {
    meter: &'a mut Meter<'b>,
}
impl<'de> DeserializeSeed<'de> for OneFact<'_, '_> {
    type Value = PricedMeasurement;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        self.meter.facts(1).map_err(serde::de::Error::custom)?;
        PricedMeasurement::deserialize(d)
    }
}
fn exact_operation(operation: &Operation, target: TurnTarget<'_>) -> bool {
    operation.thread_id.as_ref() == target.thread
        && operation.turn_id.as_deref() == Some(target.turn)
}
struct Operations<'a, 'b> {
    meter: &'a mut Meter<'b>,
    target: TurnTarget<'a>,
}
impl<'de> DeserializeSeed<'de> for Operations<'_, '_> {
    type Value = Vec<Arc<Operation>>;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for Operations<'_, '_> {
    type Value = Vec<Arc<Operation>>;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded canonical operations")
    }
    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut out = vec![];
        while let Some(operation) = seq.next_element_seed(OneOperation(self.meter))? {
            if !exact_operation(&operation, self.target) {
                return Err(serde::de::Error::custom("operation attribution mismatch"));
            }
            out.push(Arc::new(operation));
        }
        Ok(out)
    }
}
struct OneOperation<'a, 'b>(&'a mut Meter<'b>);
impl<'de> DeserializeSeed<'de> for OneOperation<'_, '_> {
    type Value = Operation;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        self.0.facts(1).map_err(serde::de::Error::custom)?;
        self.0.work(1).map_err(serde::de::Error::custom)?;
        Operation::deserialize(d)
    }
}
struct TurnSlice<'a, 'b> {
    meter: &'a mut Meter<'b>,
    target: TurnTarget<'a>,
}
impl<'de> DeserializeSeed<'de> for TurnSlice<'_, '_> {
    type Value = CanonicalTurnFacts;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for TurnSlice<'_, '_> {
    type Value = CanonicalTurnFacts;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("canonical turn data")
    }
    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut measurements = None;
        let mut operations = None;
        while let Some(key) = map.next_key::<String>()? {
            self.meter.check().map_err(serde::de::Error::custom)?;
            match key.as_str() {
                "measurements" if measurements.is_none() => {
                    measurements = Some(map.next_value_seed(Facts {
                        meter: self.meter,
                        target: self.target,
                    })?)
                }
                "operations" if operations.is_none() => {
                    operations = Some(map.next_value_seed(Operations {
                        meter: self.meter,
                        target: self.target,
                    })?);
                }
                _ => {
                    return Err(serde::de::Error::custom(
                        "unexpected or duplicate turn field",
                    ));
                }
            }
        }
        Ok(CanonicalTurnFacts {
            measurements: measurements
                .ok_or_else(|| serde::de::Error::missing_field("measurements"))?,
            operations: operations.ok_or_else(|| serde::de::Error::missing_field("operations"))?,
        })
    }
}
impl Snapshot {
    pub(super) fn timing_measurements(
        &self,
        owner: &ThreadEntry,
        entry: &TurnEntry,
        target: TurnTarget<'_>,
        meter: &mut Meter<'_>,
    ) -> Result<CanonicalTurnFacts> {
        meter.check()?;
        if self.has_stored_ledger() {
            let mut out = Vec::new();
            self.visit_ledger_groups(
                chrono_tz::UTC,
                Some(target.thread),
                Some(target.turn),
                |group, read| {
                    meter.facts(group.selected_slots(Some(target.turn)).len())?;
                    meter.check()?;
                    for row in read()? {
                        serde_json::to_writer(Writer(meter), &row)
                            .map_err(|e| failure(meter, e))?;
                        if !exact(&row.fact, target) {
                            return Err(corrupt("轮次计量归属不匹配"));
                        }
                        out.push(row);
                    }
                    Ok(())
                },
            )?;
            let data = self
                .memory_turns
                .as_ref()
                .and_then(|t| t.get(&(target.thread.into(), target.turn.into())))
                .ok_or_else(|| corrupt("轮次计量索引缺失"))?;
            meter.facts(data.cold_operations.len() + data.operations.len())?;
            let operations =
                self.turn_operations_budgeted(target.thread, target.turn, data, |bytes| {
                    meter.bytes(bytes)
                })?;
            // Match the complete turn envelope, operation output and trailing
            // newline charged by the in-memory/file paths, in addition to frames.
            meter.bytes(
                (b"{\"measurements\":[".len()
                    + b"],\"operations\":".len()
                    + b"}\n".len()
                    + out.len().saturating_sub(1)) as u64,
            )?;
            serde_json::to_writer(Writer(meter), &operations).map_err(|e| failure(meter, e))?;
            for operation in &operations {
                meter.work(1)?;
                if !exact_operation(operation, target) {
                    return Err(corrupt("轮次操作归属不匹配"));
                }
            }
            return Ok(CanonicalTurnFacts {
                measurements: out,
                operations,
            });
        }
        if let Some(turns) = &self.memory_turns {
            let data = turns
                .get(&(target.thread.into(), target.turn.into()))
                .ok_or_else(|| corrupt("轮次计量索引缺失"))?;
            let rows = self
                .live_rows
                .as_ref()
                .ok_or_else(|| corrupt("轮次计量事实缺失"))?;
            meter.facts(
                data.measurements
                    .len()
                    .checked_add(data.operations.len() + data.cold_operations.len())
                    .ok_or_else(|| corrupt("轮次记录数量溢出"))?,
            )?;
            // Charge counts before decoding cold frames; target output remains bounded.
            let operation_rows =
                self.turn_operations_budgeted(target.thread, target.turn, data, |bytes| {
                    meter.bytes(bytes)
                })?;
            let value = LiveTurn {
                measurements: LiveRows {
                    indices: &data.measurements,
                    rows,
                },
                operations: &operation_rows,
            };
            serde_json::to_writer(Writer(meter), &value).map_err(|e| failure(meter, e))?;
            meter.bytes(1)?; // Persisted slices include one trailing newline.
            let mut out = Vec::with_capacity(data.measurements.len());
            for index in &data.measurements {
                meter.check()?;
                let row = rows
                    .get(*index)
                    .ok_or_else(|| corrupt("轮次计量索引越界"))?;
                if !exact(&row.fact, target) {
                    return Err(corrupt("轮次计量归属不匹配"));
                }
                out.push(row.as_ref().clone());
            }
            let mut operations = Vec::with_capacity(operation_rows.len());
            for operation in &operation_rows {
                meter.work(1)?;
                if !exact_operation(operation, target) {
                    return Err(corrupt("轮次操作归属不匹配"));
                }
                operations.push(operation.clone());
            }
            return Ok(CanonicalTurnFacts {
                measurements: out,
                operations,
            });
        }
        meter.bytes(entry.slice.length)?;
        let mut file = fs::File::open(safe_file(&self.directory, &owner.file.file)?)
            .map_err(|e| corrupt(format!("轮次计量分片无法读取：{e}")))?;
        if entry
            .slice
            .offset
            .checked_add(entry.slice.length)
            .is_none_or(|end| end > file.metadata().map(|m| m.len()).unwrap_or(0))
        {
            return Err(corrupt("轮次定位越界"));
        }
        file.seek(SeekFrom::Start(entry.slice.offset))?;
        let size = usize::try_from(entry.slice.length).map_err(|_| corrupt("轮次长度超出范围"))?;
        let mut bytes = vec![0; size];
        let mut hash = Sha256::new();
        for chunk in bytes.chunks_mut(64 * 1024) {
            meter.check()?;
            file.read_exact(chunk)
                .map_err(|e| corrupt(format!("轮次计量分片无法读取：{e}")))?;
            hash.update(&*chunk);
        }
        meter.check()?;
        if format!("{:x}", hash.finalize()) != entry.slice.sha256 {
            return Err(corrupt("轮次分片校验失败"));
        }
        let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
        let out = TurnSlice { meter, target }
            .deserialize(&mut deserializer)
            .map_err(|e| failure(meter, e))?;
        deserializer
            .end()
            .map_err(|e| corrupt(format!("轮次计量尾部无效：{e}")))?;
        meter.check()?;
        Ok(out)
    }
}
