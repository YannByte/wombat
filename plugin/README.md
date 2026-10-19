# Wombat plugin source

English | [中文](README.zh-CN.md)

This directory contains the user plugin source. Each Skill serves Chinese and English. Codex manages installed plugins.

## Source entries

- `skills/` contains the [query and task entries](../docs/guides/plugin.en.md) and shared runtime references.
- `hooks/` contains collection declarations.
- `scripts/` contains the POSIX collection bridge.
- `package.json` defines plugin metadata, shipped Skill names and required capabilities. The root manifest owns the version.

## Limits

Build the source before local plugin installation. Plugin installation, discovery, Hook trust, event receipt, and data readiness require separate evidence. Windows Hook collection remains unverified.

## Read next

- [Installation guide](../docs/guides/installation.en.md): install or update the runtime and plugin.
- [Plugin guide](../docs/guides/plugin.en.md): invocation, collection, and removal.
- [Plugin development](../docs/development/plugin.en.md): build assets, local trials, and native verification.
- [Plugin instructions](AGENTS.md): maintenance rules.
