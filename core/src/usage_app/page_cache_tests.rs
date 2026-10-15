use super::*;
use serde_json::json;

fn fixture() -> Snapshot {
    let ids: Vec<_> = (0..80).map(|n| format!("task-{n:03}")).collect();
    let rows: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(n, id)| {
            (
                id.as_str(),
                id.as_str(),
                if n % 2 == 0 {
                    "2026-10-01T01:00:00Z"
                } else {
                    "2026-10-02T01:00:00Z"
                },
                n as u64,
            )
        })
        .collect();
    inspection::tests::snapshot(&rows, "page-cache")
}
fn request(action: &str, presentation: Option<&str>, offset: usize, limit: usize) -> Request {
    let mut v = json!({"action":action,"scope":{"allTime":true,"timezone":"UTC"},"offset":offset,"limit":limit,"compact":true});
    if let Some(presentation) = presentation {
        v["presentation"] = json!(presentation);
    }
    serde_json::from_value(v).unwrap()
}
#[test]
fn compact_page_windows_preserve_uncached_results_totals_order_and_boundaries() {
    let snapshot = fixture();
    for (action, presentation) in [
        ("usage", None),
        ("usage", Some("distribution")),
        ("usage", Some("models")),
        ("usage", Some("projects")),
        ("threads", None),
        ("statistics", None),
        ("statistics", Some("projects")),
        ("statistics", Some("models")),
    ] {
        let mut defaults = request(action, presentation, 0, 50);
        defaults.offset = None;
        defaults.limit = None;
        let mut expected = execute_uncached(defaults.clone(), &snapshot).unwrap();
        compact::apply(&mut expected);
        assert_eq!(
            serde_json::to_value(execute_snapshot(defaults, &snapshot).unwrap()).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        for offset in [0, 1, 20, 49, 50, 59, 79, 80, 100] {
            for limit in [1, 7, 21, 40, 50, 60] {
                let r = request(action, presentation, offset, limit);
                let mut expected = execute_uncached(r.clone(), &snapshot).unwrap();
                compact::apply(&mut expected);
                let actual = execute_snapshot(r, &snapshot).unwrap();
                assert_eq!(
                    serde_json::to_value(actual).unwrap(),
                    serde_json::to_value(expected).unwrap(),
                    "{action} {presentation:?} {offset} {limit}"
                );
            }
        }
    }
}
#[test]
fn a_cached_window_answers_new_page_sizes_without_reading_expired_facts() {
    let mut snapshot = fixture();
    let mut prefix = execute_uncached(request("threads", None, 0, 50), &snapshot).unwrap();
    compact::apply(&mut prefix);
    let encoded =
        crate::query_cache::QueryCache::encode("window", &prefix).expect("task window budget");
    let decoded: Response = ciborium::from_reader(encoded.as_slice()).expect("task response codec");
    assert_eq!(
        serde_json::to_value(decoded).unwrap(),
        serde_json::to_value(&prefix).unwrap()
    );
    assert!(
        crate::query_cache::QueryCache::encode("window", &prefix).is_some(),
        "window encoding is {} bytes",
        encoded.len()
    );
    let large = execute_snapshot(request("threads", None, 0, 40), &snapshot).unwrap();
    let expected = execute_snapshot(request("threads", None, 3, 7), &snapshot).unwrap();
    assert_eq!(expected.items.len(), 7);
    assert_eq!(large.page.total, 80);
    snapshot.defer_details(); // End the read lease; any uncached reduction must fail.
    let actual = execute_snapshot(request("threads", None, 3, 7), &snapshot).unwrap();
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    let size_change = execute_snapshot(request("threads", None, 0, 21), &snapshot).unwrap();
    assert_eq!(size_change.items.len(), 21);
    assert_eq!(size_change.page.next_offset, Some(21));
    for r in [
        request("threads", None, 50, 7),
        request("threads", None, 40, 20),
    ] {
        let error = execute_snapshot(r, &snapshot).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "VIEW_EXPIRED"
        );
    }
    let mut invalid = request("threads", None, 0, 0);
    assert!(execute_snapshot(invalid.clone(), &snapshot).is_err());
    invalid.limit = Some(7);
    invalid.scope.model = Some("different".into());
    assert!(execute_snapshot(invalid, &snapshot).is_err());
}
