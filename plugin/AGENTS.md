# User Plugin Instructions

Follow [root rules](../AGENTS.md). This directory owns user plugin resources; .agents/skills/ owns development workflows.

- Ship only plugin/skills/wombat/SKILL.md with matching name and a precise description. Load references only for relevant tasks; colocate Skill metadata and resources.
- Reuse public CLI/contracts; keep business behavior in its module and standing rules in AGENTS.md.
- Generate manifests from plugin/package.json and the root version. Preserve skills/<name>/ in the single formal plugin. Source ownership does not prove discovery.
- One Skill serves Chinese and English. Follow the user language; use shared locale copy. Preserve protocol values and source text.
- Use synthetic examples, no credentials or machine-specific paths. Public prose follows the [bilingual workflow](../docs/i18n/workflow.en.md).
- Run skills:check and repo:check; validate installation, initialization and real task behavior separately.

- Optional Hook receipt stays in the runtime for user-managed integrations. Ship no collection plugin, bridge or automatic Hook registration.
