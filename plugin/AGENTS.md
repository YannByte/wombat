# User Plugin Instructions

Follow [root rules](../AGENTS.md). This directory owns user plugin resources; .agents/skills/ owns development workflows.

- Ship only plugin/skills/wombat/SKILL.md with matching name and a precise description. Load references only for relevant tasks; colocate Skill metadata and resources.
- Reuse public CLI/contracts; keep business behavior in its module and standing rules in AGENTS.md.
- Generate manifests from plugin/package.json and the root version. Preserve skills/<name>/ in plugins; extract standalone resources from the same source. Source ownership does not prove discovery.
- One Skill serves Chinese and English. Follow the user language; use shared locale copy. Preserve protocol values and source text.
- Use synthetic examples, no credentials or machine-specific paths. Public prose follows the [bilingual workflow](../docs/i18n/workflow.en.md).
- Run skills:check and repo:check; validate installation, initialization and real task behavior separately.

- hooks/ and scripts/ own collection declarations and the POSIX bridge. Include only in the collection package; parsing, storage and status stay in the runtime.
