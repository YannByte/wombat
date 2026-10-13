//! Borrowed live facts and exact persisted turn reads.
use super::*;
impl Snapshot {
    pub(crate) fn is_live(&self) -> bool {
        self.live_rows.is_some()
    }
    pub(crate) fn export_live(&self) -> Result<Snapshot> {
        let prices = crate::pricing_sync::current()?;
        if prices.catalog_hash != self.manifest.price_catalog_hash {
            return Err(operation_error(
                "PRICE_CHANGED",
                "价表已经更新，请重新同步后保存",
            ));
        }
        let collected = self.live_collected()?;
        save_with_prices(&product_home()?, collected, prices)
    }
    pub(super) fn live_collected(&self) -> Result<Collected> {
        self.live_rows
            .as_ref()
            .ok_or_else(|| operation_error("INVALID_ARGUMENT", "只能导出实时读取版本"))?;
        let turns = self.memory_turns.as_ref().unwrap();
        let loaded: std::collections::HashSet<_> = self
            .manifest
            .threads
            .iter()
            .map(|t| t.thread.id.as_str())
            .collect();
        let collected = Collected {
            sources: self.manifest.sources.clone(),
            title_observations: self.manifest.title_observations.clone(),
            watermarks: self.manifest.watermarks.clone(),
            issues: self.manifest.issues.clone(),
            threads: self
                .manifest
                .threads
                .iter()
                .map(|e| e.thread.clone())
                .collect(),
            turns: self
                .manifest
                .threads
                .iter()
                .flat_map(|e| e.turns.values().filter_map(|t| t.turn.clone()))
                .collect(),
            events: self.events()?,
            measurements: self.ledger()?.iter().map(|r| r.fact.clone()).collect(),
            operations: turns
                .iter()
                .filter(|((thread, _), _)| loaded.contains(thread.as_str()))
                .map(|((thread, turn), data)| self.turn_operations(thread, turn, data))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect(),
        };
        Ok(collected)
    }
    pub(crate) fn resident_operation_facts(&self) -> impl Iterator<Item = &Arc<Operation>> {
        self.memory_turns
            .iter()
            .flat_map(|turns| turns.values().flat_map(|t| t.operations.iter()))
    }
    pub(crate) fn operation_facts(&self) -> Result<Vec<Arc<Operation>>> {
        let loaded: std::collections::HashSet<_> = self
            .manifest
            .threads
            .iter()
            .map(|t| t.thread.id.as_str())
            .collect();
        let mut out = Vec::new();
        let mut locations = Vec::new();
        for ((thread, _), turn) in self.memory_turns.iter().flat_map(|t| t.iter()) {
            if loaded.contains(thread.as_str()) {
                out.extend(turn.operations.iter().cloned());
                locations.extend(turn.cold_operations.iter().cloned());
            }
        }
        if !locations.is_empty() {
            let reader = self
                .cold_events
                .as_ref()
                .ok_or_else(|| corrupt("冷操作读取版本缺失"))?;
            let mut touched = 0_u64;
            out.extend(reader.operations(&locations, |bytes| {
                touched = touched
                    .checked_add(bytes)
                    .ok_or_else(|| operation_error("RESOURCE_LIMIT", "冷操作读取大小溢出"))?;
                if touched > MAX_FILE_BYTES {
                    return Err(operation_error("RESOURCE_LIMIT", "操作读取范围过大"));
                }
                Ok(())
            })?);
        }
        Ok(out)
    }
    pub(crate) fn measurement_facts(&self) -> impl Iterator<Item = &Arc<Measurement>> {
        self.live_rows
            .iter()
            .flat_map(|rows| rows.iter().map(|r| &r.fact))
    }
    pub(crate) fn live_ledger(&self) -> Option<Vec<&PricedMeasurement>> {
        if self.has_stored_ledger() {
            return None;
        }
        let rows = self.live_rows.as_ref()?;
        if self.project_loads.is_empty() {
            return Some(rows.iter().map(Arc::as_ref).collect());
        }
        let projects = self.project_rows.as_ref()?;
        let mut selected: Vec<_> = self
            .project_loads
            .iter()
            .filter(|p| p.state == crate::live::ProjectLoadState::Ready)
            .flat_map(|p| projects.get(&p.project).into_iter().flatten().copied())
            .collect();
        selected.sort_unstable();
        Some(selected.into_iter().map(|i| rows[i].as_ref()).collect())
    }
    pub(crate) fn live_scope_ledger(
        &self,
        scope: &crate::usage_app_dto::Scope,
    ) -> Option<Vec<&PricedMeasurement>> {
        if self.has_stored_ledger() {
            return None;
        }
        if scope.project.is_none() && scope.project_unknown != Some(true) {
            return self.live_ledger();
        }
        let rows = self.live_rows.as_ref()?;
        let projects = self.project_rows.as_ref()?;
        if !self.project_loads.is_empty()
            && !self.project_loads.iter().any(|p| {
                p.project == scope.project && p.state == crate::live::ProjectLoadState::Ready
            })
        {
            return Some(vec![]);
        }
        Some(
            projects
                .get(&scope.project)
                .into_iter()
                .flatten()
                .map(|index| rows[*index].as_ref())
                .collect(),
        )
    }
    pub(crate) fn live_detail_rows(
        &self,
        thread_id: &str,
        turn_id: Option<&str>,
    ) -> Result<Option<Vec<&PricedMeasurement>>> {
        if self.has_stored_ledger() {
            return Ok(None);
        }
        let (Some(turns), Some(rows)) = (&self.memory_turns, &self.live_rows) else {
            return Ok(None);
        };
        let thread = self
            .manifest
            .threads
            .iter()
            .find(|t| t.thread.id == thread_id)
            .ok_or_else(|| operation_error("NOT_FOUND", "未找到对话"))?;
        let ids = if let Some(turn_id) = turn_id {
            if !thread.turns.contains_key(turn_id) {
                return Err(operation_error("NOT_FOUND", "未找到轮次"));
            }
            vec![turn_id]
        } else {
            thread.turns.keys().map(String::as_str).collect()
        };
        Ok(Some(
            ids.into_iter()
                .filter_map(|id| turns.get(&(thread_id.into(), id.into())))
                .flat_map(|turn| turn.measurements.iter().map(|index| rows[*index].as_ref()))
                .collect(),
        ))
    }
    pub fn ledger(&self) -> Result<Vec<PricedMeasurement>> {
        if self.has_stored_ledger() {
            return self.stored_measurements(None, None);
        }
        if self.live_rows.is_some() {
            return Ok(self.live_ledger().unwrap().into_iter().cloned().collect());
        }
        let bytes = bounded_read(&safe_file(&self.directory, &self.manifest.ledger.file)?)?;
        if crate::hash(&bytes) != self.manifest.ledger.sha256 {
            return Err(corrupt("计量分片校验失败"));
        }
        let rows: Vec<PricedMeasurement> =
            serde_json::from_slice(&bytes).map_err(|e| corrupt(format!("计量分片无效：{e}")))?;
        validate_measurements(&rows)?;
        Ok(rows)
    }
    /// Inspection already owns its scoped ledger. Read operation facts without
    /// selecting and cloning that turn's measurements a second time.
    pub(crate) fn operations_for_turn(
        &self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<Vec<Arc<Operation>>> {
        let thread = self
            .manifest
            .threads
            .iter()
            .find(|t| t.thread.id == thread_id)
            .ok_or_else(|| operation_error("NOT_FOUND", "未找到对话"))?;
        if !thread.turns.contains_key(turn_id) {
            return Err(operation_error("NOT_FOUND", "未找到轮次"));
        }
        if let Some(turns) = &self.memory_turns {
            let data = turns
                .get(&(thread_id.into(), turn_id.into()))
                .ok_or_else(|| operation_error("NOT_FOUND", "未找到轮次"))?;
            return self.turn_operations(thread_id, turn_id, data);
        }
        Ok(self
            .turn(thread_id, turn_id)?
            .operations
            .into_iter()
            .map(Arc::new)
            .collect())
    }
    pub fn turn(&self, thread_id: &str, turn_id: &str) -> Result<TurnData> {
        if self.has_stored_ledger() {
            let data = self
                .memory_turns
                .as_ref()
                .and_then(|t| t.get(&(thread_id.into(), turn_id.into())))
                .ok_or_else(|| operation_error("NOT_FOUND", "未找到轮次"))?;
            return Ok(TurnData {
                measurements: self.stored_measurements(Some(thread_id), Some(turn_id))?,
                operations: self
                    .turn_operations(thread_id, turn_id, data)?
                    .iter()
                    .map(|op| op.as_ref().clone())
                    .collect(),
            });
        }
        if let Some(turns) = &self.memory_turns {
            if !self
                .manifest
                .threads
                .iter()
                .any(|t| t.thread.id == thread_id)
            {
                return Err(operation_error("NOT_FOUND", "未找到对话"));
            }
            let turn = turns
                .get(&(thread_id.into(), turn_id.into()))
                .ok_or_else(|| operation_error("NOT_FOUND", "未找到轮次"))?;
            let rows = self.live_rows.as_ref().unwrap();
            return Ok(TurnData {
                measurements: turn
                    .measurements
                    .iter()
                    .map(|id| rows[*id].as_ref().clone())
                    .collect(),
                operations: self
                    .turn_operations(thread_id, turn_id, turn)?
                    .iter()
                    .map(|op| op.as_ref().clone())
                    .collect(),
            });
        }
        let thread = self
            .manifest
            .threads
            .iter()
            .find(|t| t.thread.id == thread_id)
            .ok_or_else(|| operation_error("NOT_FOUND", "未找到对话"))?;
        let entry = thread
            .turns
            .get(turn_id)
            .ok_or_else(|| operation_error("NOT_FOUND", "未找到轮次"))?;
        if entry.slice.length > MAX_FILE_BYTES {
            return Err(corrupt("轮次分片过大"));
        }
        let mut file = fs::File::open(safe_file(&self.directory, &thread.file.file)?)?;
        if entry
            .slice
            .offset
            .checked_add(entry.slice.length)
            .is_none_or(|end| end > file.metadata().map(|m| m.len()).unwrap_or(0))
        {
            return Err(corrupt("轮次定位越界"));
        }
        file.seek(SeekFrom::Start(entry.slice.offset))?;
        let mut bytes = vec![0; entry.slice.length as usize];
        file.read_exact(&mut bytes)?;
        if crate::hash(&bytes) != entry.slice.sha256 {
            return Err(corrupt("轮次分片校验失败"));
        }
        let data: TurnData =
            serde_json::from_slice(&bytes).map_err(|e| corrupt(format!("轮次分片无效：{e}")))?;
        validate_measurements(&data.measurements)?;
        Ok(data)
    }
}

impl Snapshot {
    /// Project progress masks one already-prepared immutable view. Facts, pricing,
    /// membership and aggregate indexes are shared rather than rebuilt per batch.
    pub(crate) fn project_view(&self, id: String, progress: Vec<crate::live::ProjectLoad>) -> Self {
        let ready: std::collections::BTreeSet<_> = progress
            .iter()
            .filter(|p| p.state == crate::live::ProjectLoadState::Ready)
            .map(|p| &p.project)
            .collect();
        let threads: Vec<_> = self
            .manifest
            .threads
            .iter()
            .filter(|t| ready.contains(&t.thread.project))
            .cloned()
            .collect();
        let loaded: std::collections::BTreeSet<_> =
            threads.iter().map(|t| t.thread.id.as_str()).collect();
        // Select shared immutable owners directly; cloning a complete manifest
        // before masking also copies every turn and event locator transiently.
        let manifest_ref = &self.manifest;
        let mut manifest = Manifest {
            schema_version: manifest_ref.schema_version,
            observation_versions: manifest_ref.observation_versions,
            title_observations: manifest_ref
                .title_observations
                .iter()
                .filter(|o| loaded.contains(o.thread_id.as_str()))
                .cloned()
                .collect(),
            snapshot_ref: SnapshotRef {
                snapshot_id: id,
                created_at: manifest_ref.snapshot_ref.created_at.clone(),
            },
            price_revision: manifest_ref.price_revision.clone(),
            price_catalog_hash: manifest_ref.price_catalog_hash.clone(),
            sources: manifest_ref.sources.clone(),
            watermarks: manifest_ref.watermarks.clone(),
            issues: manifest_ref.issues.clone(),
            ledger: manifest_ref.ledger.clone(),
            events: EventIndex {
                version: manifest_ref.events.version,
                partitions: manifest_ref
                    .events
                    .partitions
                    .iter()
                    .filter(|p| {
                        p.target
                            .thread_id
                            .as_deref()
                            .is_none_or(|t| loaded.contains(t))
                    })
                    .cloned()
                    .collect(),
            },
            threads,
        };
        let issue = Issue {
            code: "projectRestoreIncomplete".into(),
            message: "已加载的项目可以查看，其他项目正在依次加载".into(),
            source_instance_id: None,
            evidence: None,
        };
        manifest.issues.push(issue.clone());
        for source in &mut manifest.sources {
            if source.status == "complete" {
                source.status = "partial".into();
            }
            source.issues.push(issue.clone());
        }
        Self {
            summary_index: Arc::clone(&self.summary_index),
            publication_change: None,
            project_loads: progress,
            timing_cache: Mutex::default(),
            query_cache: Mutex::default(),
            query_work: Arc::clone(&self.query_work),
            live_rows: self.live_rows.clone(),
            project_rows: self.project_rows.clone(),
            live_events: None,
            cold_events: self.cold_events.clone(),
            memory_turns: self.memory_turns.clone(),
            manifest,
            directory: self.directory.clone(),
        }
    }
}

pub(super) fn validate_measurements(rows: &[PricedMeasurement]) -> Result<()> {
    for row in rows {
        crate::adapters::contract::validate_token_observations(
            &row.fact.tokens,
            &row.fact.token_unavailable_reasons,
        )
        .map_err(|e| corrupt(format!("计量观察无效：{e}")))?;
    }
    Ok(())
}
