# Third-party notices

Wombat's original code is licensed under [MIT](../LICENSE). Third-party components retain their original licenses; the root license does not relicense them.

- Node dependency notices: [license texts](node-dependencies.txt).
- Rust dependency notices: [license texts](rust-dependencies.txt).
- Versions, declared licenses, selected alternatives and notice hashes: [inventory](../docs/dependency-licenses.json).
- GitHub Release archives also include the exact Node.js 26.4.0 distribution license as `lib/licenses/node-runtime.txt`; each platform export hashes that file together with the bundled runtime binary.

The inventory covers 122 installed Node packages (9 runtime) and 191 locked Rust packages other than Wombat. Enabled normal/build/dev dependencies are identified separately; optional lock entries not activated by current features are not compiled into this build, but their upstream license notices are still preserved. Node optional platform packages not installed here are recorded as unverified; their presence is not a platform support claim. Development dependencies are included conservatively.

For Rust dual-licensed packages, Wombat uses the offered MIT alternative where available, retaining additional required notices, including Unicode-3.0. The alternative LGPL license offered by r-efi is not selected. Its AUTHORS file is preserved. Binary Node packages without their own license text retain the same-version upstream package notice. Crates that omit license texts retain supplemental notices from recorded upstream revisions, with fixed hashes.

Dependency updates require a license review and regeneration. Run `corepack pnpm licenses:generate`, then `corepack pnpm licenses:check`. Both read local metadata only; an incomplete dependency cache fails explicitly. No dependency installation, upgrade, or network download is performed by these commands.

This inventory records source declarations and notices, not a legal warranty or verification of all platform binaries.
