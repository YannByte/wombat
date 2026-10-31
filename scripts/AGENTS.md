# Development Script Instructions

Follow [root instructions](../AGENTS.md). This directory owns checks, generators, and assembly, not product rules.

- Use TypeScript and the Node toolchain. Justify executable `.mjs` exceptions; migrations preserve validation/evidence.
- Checks are read-only by default; generation, confirmation updates, and publication have explicit entry points.
- Source checks require locked dependencies, without dist, real Agent logs, or user configuration. Artifact checks require a build; missing artifacts fail acceptance.
- Wire checks into root package.json aggregates and CI; standalone scripts do not establish acceptance.
- Changed acceptance rules need isolated synthetic valid/invalid fixtures, nonzero failure exits, and actionable diagnostics. Never mutate the real repository to manufacture failures.
- Bound subprocess time/output and clean temporary directories. Do not turn failures into success or disable rules globally; justify object-specific exceptions.
- Automate computable steps/checks behind one package entry. Reuse evidence and report stages. Git/external mutations resume by revalidating identities; never replace published identities.
- Use native absolute paths in filesystem fixtures; declare foreign path grammar in recorded-path tests. Test tooling on every target.
- test:repo runs *.test.ts and *.test.mjs for tools; product and install checks are separate.
