## Establish scope

- Distinguish local builds, development candidates, five-platform candidates, and public Releases. Candidate preparation does not authorize tags, publication, or repository visibility changes. Continue under existing explicit authorization; otherwise finish the reviewable candidate before requesting approval of its version, commit, and archives.
- Inspect branch, commit, staged and unstaged changes. Use isolation when the shared checkout is changing and record the baseline and patches. Preserve user changes and exclude real logs, credentials, and private material.
- Read source Node requirements from engines.node, bundled runtime identity from scripts/github-release.ts, and Rust from rust-toolchain.toml. Install using the lockfile.
- After a GitHub account or repository rename, verify the canonical remote identity and align origin, package metadata, installers, updater, plugin metadata, release-copy generation, and public links before the next candidate. Check existing immutable Releases and public install/update routes under the new identity; redirects alone do not establish raw-file access or attestation validity. Preserve historical evidence and never rewrite published assets.
- Use release:prepare for version changes across manifests, current release docs, bilingual records, and license inventory. Preparation, commits, tags, Releases, and repository visibility are separate actions.

## Existing entries

| Goal | Entry | Evidence scope |
|---|---|---|
| Prepare version | `corepack pnpm release:prepare -- --version <version>` | Updates versions, current release docs, named bilingual records, and licenses; no commit/tag/publication |
| Check preparation | `corepack pnpm release:prepare -- --check` | Fast version, pairing, license, and diff checks; not full release acceptance |
| Generate Release notes | `corepack pnpm release:notes -- --set <release-set.json> --output <notes.md>` | Produces the canonical English body from reviewed user impact plus verified version, source, targets, and comparison facts |
| Pre-tag remote preflight | `corepack pnpm release:preflight -- --version <version>` | Requires current root READMEs, a clean `main`, exact `origin/main`, no existing tag/Release, a public unarchived repository, and successful CI for the exact commit; no mutation |
| Publish end to end | `corepack pnpm release:publish` | Uses root version; synchronizes/checks/commits/pushes, waits exact-source CI, reuses its verified archives, tags/publishes and verifies public install/update |
| Local build | `corepack pnpm build` | Compiles core, client and CLI |
| Source-only checks | `corepack pnpm test:repo` and `corepack pnpm test:prebuild` | Script and platform-assumption tests without dist; CI requires Linux, macOS, and Windows success before compilation |
| Startup probe without scanning | `corepack pnpm release:probe` | Starts the shared service with nonexistent source directories and reads capabilities to check process startup, sockets, and the CLI protocol; requires built dist/ |
| Full release gate | `corepack pnpm release:check` | Source rules and platform assumptions first; then Rust lint, build, types, contracts, product tests, and licenses |
| Focused install candidate | `corepack pnpm release:verify-install` | Builds the current target, checks types/repository rules/update regressions, and packages/tests installation and upgrade; no publication or full release signoff |
| Local candidate | `corepack pnpm github:pack -- --current-platform` | Packs and extracts the local archive, checksums, and release-set; no upload |
| Five-platform candidate | `corepack pnpm github:pack -- --native-dir <artifacts>` | Validates matching version/commit, cores, runtimes, and notices before packing; no upload |
| Native export | `corepack pnpm native:export` | Exports local core, Node runtime, licenses, version, commit, and hashes |

Packing runs release:check by default. Use --reuse-build only after gates passed on exactly the same source; fingerprint validation still applies. The focused install entry uses it after its scoped checks for candidate diagnosis, while public releases still require the full gate. Rebuild after source, lockfile, release-script, or manifest changes. If local Node lacks its distributed LICENSE, pass --runtime-license with the official license for the bundled runtime; CI setup-node should provide it. The project MIT license cannot replace Node's license.

## Review failure-class coverage before release

Follow the [failure investigation rules](../../../../docs/development/workflow.en.md#failure-investigation) before expensive platform runs. Review the following independent regressions when changing the corresponding fixtures or tools; a local pass does not establish another operating system's behavior.

| Assumption | Regression owner |
|---|---|
| Timestamp precision and date boundaries | [Time precision checks](../../../../tests/prebuild/time-precision.test.ts) |
| Process identity, deadline races, and unanswered probes | [Native lifecycle checks](../../../../tests/prebuild/native-lifecycle.test.ts) |
| CLI and single-Skill delivery | [Delivery boundary checks](../../../../scripts/check-delivery.test.ts) |
| Data readiness and version-pinned final counts | [Configuration follow-up acceptance](../../../../tests/e2e/config-version-follow-up.test.ts) |
| Standalone checks actually execute, need no build, and reject failures | [Prebuild entry acceptance](../../../../scripts/prebuild-gate.test.ts) |

Run source-only checks first. Let exact-source CI establish all operating systems, product flows, and archive installation/upgrade before publication. Publish those verified archives through the [primary publication entry](publication.md), and resume verified stages and downloads after external transport failures rather than rebuilding or replacing identities. Keep evidence in external reports and CI; do not add a second release ledger.

## Verify candidates

1. Run the scoped entry with locked dependencies in isolation. For failures, follow the [failure investigation rules](../../../../docs/development/workflow.en.md#failure-investigation) across all matching CI and release entries before another platform run; never bypass gates.
2. Inspect dist/github/release-set.json, SHA256SUMS, and every archive against the target and payload definitions in scripts/native-platforms.ts and scripts/github-release.ts. Check release.json and project, dependency, and runtime notices.
3. verify-github-release.ts uses the bundled runtime for version, read-only offline `wombat system doctor`, empty-snapshot, live, append, fixed-snapshot, task tests with an empty application PATH, synthetic sources, and temporary data directories. It also installs the preceding public release and upgrades that managed installation to the candidate, preserving the previous version while switching the current pointer.
4. Run scripts/install/install.sh --base-url file://<dist/github> into a repository-external prefix. Verify the managed marker, version directory, current.txt, launcher, and empty-PATH operation. Verify scripts/install/install.ps1 -BaseUrl <URL> on Windows itself.
5. cli/tests/update.test.ts covers check-only, size/hash, archive safety, and atomic switching with a simulated Release. Candidate CI performs the real cross-version upgrade on all five targets. After publication, test `wombat system update --check` through the stable public endpoint.
6. Report archive paths, size, SHA-256, source commit, verified platforms, and omissions. Changed archives or rebuilds invalidate previous hash/install evidence.
