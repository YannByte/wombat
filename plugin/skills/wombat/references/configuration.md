# Configuration evidence

Use native project reads for a direct file question. Use Wombat inventory when the user needs measured configuration, related tasks, or historical loading/use evidence. Keep the exact selected project and authorized source roots. Start with at most three relevant items, using one `config` request for several kinds.

Use `optimize list` for implemented static checks and `optimize checks` for per-rule facts. Findings are evidence for Codex review, not automatic edits. File size, absent use and a static rule match alone cannot establish waste or a loading defect. Unsupported future checks are not part of the catalog.

Usage `snapshotId` fixes usage. Configuration `readView` fixes inventory. Keep the original roots, project/source filters and view for later inventory reads. Do not mix a saved usage snapshot with a configuration read view. Use `config` for item detail, evidence or related scopes; use `optimize detail` for a selected current finding.

Retain the returned `baselineId` and `readView` before changing an existing finding. Follow [change and recheck](processing.md) for a scoped comparison. Normal inspections do not persist observations. Explicit user decisions have their own `decisionRevision`; they do not replace check outcomes.

Use `optimize inventory --help` or the relevant generated method Schema only for unfamiliar arguments. Request an original message only when configuration evidence leaves a question that could change the recommendation.
