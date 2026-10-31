# Decision note: continuous project discovery from observed records

[中文](2026-10-04-observed-project-discovery.md) | English

Status: implemented

## Problem

Historical sessions already contain reliable working directories, yet Web project switching required a separate directory addition. A direct configuration URL could fail before the Usage page built the project list. Projects first used while the host was running also required a restart or manual addition, which conflicted with continuous incremental reading.

## Decision

Project directories in the shared Rust usage result form the local Web session’s project catalog. Web merges new projects after each usage result. When Configuration, Optimize, or handoff directly requests an unknown project, the host synchronizes current sources first and accepts only a path that appears exactly in that result. Observed projects require neither separate authorization nor a restart. Extra directories absent from history still use the host system picker.

Discovery stores only derived identity and opens current configuration on demand. It does not traverse the disk for projects or execute logs or project contents. Browser-supplied `roots`, `projectRoots`, and unobserved paths remain rejected.

## Alternatives considered

- Show an authorization prompt for each historical project. The user explicitly declined authorization, and it would turn an existing source fact into a repeated setup step.
- Trust a project path supplied by the browser. That would make the local Web host an arbitrary file-read boundary, so it was rejected.
- Scan common parent directories for repositories. This expands privacy, disk, and attribution scope without proving that an Agent used the directory, so it was rejected.

## Consequences and verification

A project appears after a supported Agent first records a reliable working directory. Creating a folder alone does not add it. Source-root file notifications, periodic checks, and fresh synchronization discover changes; the persistent index restores them after restart. A configuration read still follows an explicit project navigation action, and log content never triggers commands.

Synthetic end-to-end coverage includes a project present before host startup, one added after startup, direct configuration and optimization reads, arbitrary unobserved paths, and browser-forged roots. Per-project narration and progressive commits for the full first-read experience remain separate gaps; see the [initialization design](../../rejected/product/2026-10-02-progressive-initialization.en.md).
