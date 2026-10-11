# Wombat bilingual documentation workflow

[中文](workflow.md) | English

Public documentation is maintained in Chinese and English pairs; all existing prose pages are paired. Both languages have equal authority. Either version may be edited first, with the other updated in the same change. Use the [terminology table](terminology.md) for terms and the [documentation rules](../AGENTS.md) for content ownership.

## Files and scope

Every directory uses English for `README.md` and Chinese for `README.zh-CN.md`. Other documentation keeps the existing Chinese path `docs/foo.md`, with English at `docs/foo.en.md` and the confirmation record at `docs/foo.i18n.json`. Contribution and security pages live under `.github/`; confirmation records for these repository entry pages live under `docs/i18n/records/`. Version 2 records store SHA-256 hashes for each language by section, keyed by English heading paths. Code blocks are compared verbatim and excluded from section prose hashes. Section records update independently to reduce merge conflicts. Hashes identify the confirmed content versions; they do not establish translation quality.

The [pairing manifest](../../scripts/doc-i18n.manifest.json) lists all paired pages and explicit exclusions; `legacyUnpaired` is now empty and must not become an entry point for new monolingual documents. Root-module READMEs and active Decision Notes are paired individually. New public documents must be added as complete pairs. Importing an external legacy document adds both languages and a record; an automatic translation draft is not a confirmed pair. `AGENTS.md`, the terminology table, generated schemas, and dated measurement artifacts are outside page-by-page translation scope.

## Update workflow

1. Check source code, contracts, and current verification evidence before editing the page that owns a fact; do not infer current support from historical records.
2. Update the affected passages in the other language. Preserve heading levels, list structure, commands, and code blocks. Document links should point to the available version in that language; links to exempt documents and machine-readable evidence keep the same target.
3. Review both versions using the language review guidance below, then run `corepack pnpm docs:i18n:record -- <Chinese file>`. Update records only for explicitly named, reviewed pairs.
4. Run `corepack pnpm docs:check` and `git diff --check`, then review the changes in both languages and the record.

The check uses a Markdown syntax tree to verify complete pairs, section hashes, language links, heading levels, nested lists and starting numbers, table dimensions, code blocks, and link targets including queries and fragments. It cannot determine whether both languages express the same facts or read naturally; the editor must review those aspects. The manifest lists exclusions explicitly. A passing check does not mean all original source text in the product is translated.

## Language review

Use [ASD-STE100 Issue 9](https://www.asd-ste100.org/) to write and review technical documentation in English. This includes procedures, technical references, and module guides. Review both the official writing rules and dictionary. Apply the standard's rules for technical nouns and technical verbs to project terms. The terminology table keeps both languages consistent; it does not replace the official dictionary. Apply relevant controlled writing principles to Chinese while preserving Chinese grammar and usage.

Use active voice and imperative verbs in procedures. Give each step one operation. Put conditions and necessary warnings before the action. In descriptions, give each sentence one main idea and each paragraph one topic. Keep conditions, limits, and failure recovery. Do not remove necessary information to shorten sentences. Keep code, commands, and fixed quotations unchanged.

Pairing checks do not verify ASD-STE100 compliance. Claim compliance for an English document only after review against the official writing rules and dictionary. If that review is incomplete, report its scope and the unverified items. Passing repository checks is not proof of compliance.

Translations should read naturally in the target language while preserving the same facts and constraints. Match meaning by paragraph rather than copying word order or sentence length. Sentences may be split or combined within a paragraph, provided the structure required by the pairing checks is preserved.

- In Chinese, name actions and their objects clearly. Avoid chains of abstract nouns, copied English passive constructions, and omitted subjects where they are needed. Use Chinese punctuation and appropriate spacing around English text, numbers, and inline code.
- In English, use natural subject–verb constructions and choose articles, number, tense, and prepositions for the context. Use clear verbs in instructions; avoid copying Chinese sentence fragments or stacking noun modifiers.
- Use the terminology table consistently and translate explanatory prose in context. Keep commands, fields, protocol values, stable identifiers, and original source text unchanged. Link labels may be translated, but their targets must lead to the corresponding version of the same content.
- Compare who acts, conditions, the scope of negation, quantities, units, limits, and certainty. Do not turn planned, possible, or unverified behavior into claims of delivery or verification. Finally, read each version on its own to ensure readers can understand it without consulting the other language.

## Incremental maintenance

Register localized interface images under `assetPairs` in the pairing manifest with Chinese and English paths. Checks cover file existence, unique pairing, and the language of README links; they do not judge image translations. Review image content manually; non-localized logos share the same file.

For routine edits, read the terminology table and directly update the changed passages while preserving untouched translations. Never re-record hashes without review merely to pass checks. Delete or rename both languages and their record together.

`corepack pnpm docs:i18n:check -- docs/i18n/README.md` validates the named pair’s content; repository checks still validate every pair. Failures identify changed sections and languages. `node scripts/check-doc-i18n.mjs --migrate` migrates only records whose contents still exactly match the old whole-file confirmation hashes; it does not confirm new translations. Records remain JSON, and that command does not rename files or change pairing scope.

Product language and documentation translation are maintained separately; see [product language and copy](product.en.md).
