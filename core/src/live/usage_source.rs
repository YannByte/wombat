//! Canonical accounting from a foreground source read.
use super::*;

pub(super) fn collect(
    roots: &[String],
    recent_only: bool,
    context: &RunContext,
) -> Result<Collected> {
    let facts = adapters::collect_planned(
        &adapters::registry(),
        &DiscoveryRequest {
            roots: roots.iter().map(PathBuf::from).collect(),
        },
        context,
        &ReadPlan {
            recent_only,
            usage_only: true,
        },
    );
    if context.is_cancelled() {
        return Err(operation_error(
            "CANCELLED",
            "Usage source read was cancelled",
        ));
    }
    if !facts
        .sources
        .iter()
        .any(|s| matches!(s.status.as_str(), "complete" | "partial"))
    {
        return Err(operation_error(
            "SOURCE_UNREADABLE",
            "No readable usage source",
        ));
    }
    Ok(facts)
}
