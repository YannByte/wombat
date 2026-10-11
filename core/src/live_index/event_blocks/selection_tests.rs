//! Verify target lookup work without machine-dependent elapsed-time assertions.
use super::*;
use rusqlite::StatementStatus;

#[test]
fn target_joins_use_indexed_lookups_instead_of_multiplying_targets_and_frames() {
    let directory = tempfile::tempdir().unwrap();
    let db = crate::live_index::open(&directory.path().join("index.sqlite")).unwrap();
    let owner = bucket(&db, "projection:synthetic", "events").unwrap();
    let other = bucket(&db, "projection:other", "events").unwrap();
    let mut insert = db
        .prepare("INSERT INTO event_blocks VALUES(?1,'source',?2,0,?3,1,2,'digest',x'5b5d',1)")
        .unwrap();
    let mut targets = Vec::new();
    for index in 0..1024 {
        // NULL owners and turns must select exactly like assigned targets.
        let target = if index == 0 {
            "[null,null]".to_owned()
        } else {
            serde_json::to_string(&(format!("thread-{index}"), Some("turn"))).unwrap()
        };
        insert.execute(params![owner, index, &target]).unwrap();
        insert.execute(params![other, index, &target]).unwrap();
        if index % 2 == 0 {
            targets.push(target);
        }
    }
    drop(insert);
    let target_json = serde_json::to_string(&targets).unwrap();
    let scope_json = serde_json::to_string(&["projection:synthetic"]).unwrap();
    for (sql, scope) in [
        (TARGET_ROWS, &scope_json as &dyn rusqlite::ToSql),
        (reduction::TARGET_ROWS, &owner as &dyn rusqlite::ToSql),
    ] {
        let mut query = db.prepare(sql).unwrap();
        let rows: Vec<i64> = query
            .query_map(params![scope, &target_json], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(rows.len(), targets.len());
        let steps = query.get_status(StatementStatus::VmStep);
        assert!(steps < 100_000, "target lookup work: {steps}");
        for id in rows {
            let (bucket, target): (i64, String) = db
                .query_row(
                    "SELECT bucket,target FROM event_blocks WHERE rowid=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(bucket, owner);
            assert!(targets.contains(&target));
        }
        // The former plan produces the same rows but does multiplicative work.
        let original = sql.replace("CROSS JOIN", "JOIN");
        let mut old = db.prepare(&original).unwrap();
        let rows: Vec<i64> = old
            .query_map(params![scope, &target_json], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(rows.len(), targets.len());
        let old_steps = old.get_status(StatementStatus::VmStep);
        assert!(old_steps > steps * 10);
        println!("target lookup VM steps: {steps}; former join: {old_steps}");
    }
}
