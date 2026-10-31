# Product language and copy

[中文](product.md) | English

This page defines the CLI presentation language and the rules for user-facing pages and copy. The [bilingual documentation workflow](workflow.en.md) owns document pairing.

## Usage

```sh
wombat --lang en
wombat usage --lang zh
wombat prices --lang en
WOMBAT_LANG=en wombat --help
```

Language precedence is `--lang`, `WOMBAT_LANG`, system language, then the Chinese compatibility default. Supported languages are `zh` / `en` and their regional tags. The system language is the first nonempty value of `LC_ALL`, `LC_MESSAGES`, and `LANG`, or the runtime language when none is set; `C` / `POSIX` selects English. An unsupported explicit selection is an argument error; an unmatched system language falls back to Chinese.


## User-facing pages and copy

Use these rules to write and review pages, dialogs, forms, notices, CLI help, and error messages. They are maintenance requirements, not a claim that all existing pages have passed acceptance checks. Apply ASD-STE100 to technical explanations as described in the [language review rules](workflow.en.md). Use familiar language and interface conventions for buttons, menus, and short labels. Do not add words only to make a label a complete sentence.

- Organize information around the user's task. First explain what the user is viewing, what the result means, and what they can do next. Provide methods and technical evidence as needed. Use familiar product terms by default. Show implementation details when they help users locate a problem, check evidence, or make a decision. Do not explain product actions with internal module names, protocol terms, or development status.
- Use page titles to name the object or task, buttons to name the action and its target when needed, and links to name the destination. Use the same name for the same action. Prefer specific labels such as “View details,” “Retry,” and “Clear filters” to vague labels such as “Process,” “OK,” or “Click here.” Distinguish viewing, saving, rechecking, and handing work to Codex according to their actual behavior. Do not present a recommendation as a completed action.
- In Chinese, use natural, concise verb–object phrases and Chinese punctuation. Avoid English word order, awkward passive constructions, and chains of nouns. In English, use clear action verbs and consistent sentence case for labels. Keep the spelling of proper names. Choose articles, singular or plural forms, and tone for the context rather than translating word for word. Use the presentation language's date, number, and unit formats. Keep original source text and stable identifiers unchanged.
- State the current facts and their effects. Distinguish loading, no data, no filter matches, missing permission, read failures, partial results, and previous results. Derive a value or state when available evidence supports it. Distinguish not started, in progress, failed, not recorded by the source, unsupported, and not applicable; omit information that does not apply. Keep zero, missing values, and unpriced amounts distinct. Do not invent zero values or causes when evidence is absent. Identify cost estimates and the scope of observations. In errors, first name the failed operation and affected object, then give an available recovery action. Do not infer causes without evidence or promise success or recovery times.
- Follow familiar interaction conventions. Buttons perform actions; links navigate. Give inputs explicit labels and use placeholders only for examples. Save, cancel, back, and retry must describe the actual behavior. Explain why an action is disabled when needed. Before a destructive action or one that is difficult to undo, explain its target, effects, and whether it can be undone. Confirmation copy must name the action and cannot replace authorization checks.
- Keep necessary information understandable with keyboards, narrow screens, and assistive technology. Accessible names should include the action or object in the visible label. Do not communicate status, errors, or necessary instructions only through color, icons, hover content, or placeholders. Keep messages complete when text wraps, counts change, or the language changes. Do not use English sentence assembly patterns for Chinese messages.
- Timing, tokens, and rule checks should provide useful explanations without requiring exact values. Show estimates or ranges when supported, with their method and scope. Distinguish limited precision from an unavailable conclusion. Report insufficient evidence only when missing information affects the judgment, and explain that effect. State what a rule checked and the scope of its conclusion instead of using a generic unknown state.
- Read each language on its own. Review the actual interface for information order, action names, recovery paths, long titles, zero and missing values, and singular or plural forms. Use synthetic content to check long text, narrow screens, keyboard use, and language switching. Report the states you checked. Passing dictionary and pairing checks does not replace review of usability, accessibility, or language quality.

## Module boundaries

- `@wombat/client/locale` is a separate public entry written in portable TypeScript, with no Node, React, or Rust dependency. It provides `LocaleRuntime`, `resolveLocale`, typed `t`, immutable state, and subscriptions; each CLI process uses its own presentation instance.
- `client/src/locale/zh.ts` and `en.ts` contain complete messages with feature-prefixed keys; the Chinese key set constrains the English keys. Parameter names are derived from Chinese templates, and a separate check verifies bilingual parameter parity. Values are inserted verbatim without recursive parsing or translation.
- The CLI interprets language arguments and environment variables. Module-level labels resolve lazily to avoid capturing an old language at startup. UI operations use stable identifiers and must not infer business actions from translated text.
- Language affects help, notices, labels, dates, and compact number display. JSON fields, error codes, model identifiers, source titles, user content, usage, and amounts are not translated. Known core and host progress signals are mapped at the presentation layer; unknown core diagnostics and source issues remain verbatim.
- Count messages use complete templates and native `Intl.PluralRules` for singular/plural selection; zero, unknown values and two independent counts are handled separately. Month buckets show localized calendar months without converting calendar dates to the host timezone. Configuration instruction tokens are marked as estimates. Lists use KiB (1,024 B), retain exact byte evidence and sort by raw values.

## Verification and limits

`corepack pnpm i18n:check` checks static Chinese and English dictionaries for missing, duplicate, and empty entries, interpolation-parameter parity, and mixed-language values. It also checks hard-coded copy in selected CLI presentation sinks. Source checks validate finite translation keys and `@wombat/client/locale` bindings. They follow same-file constants, no-argument helper returns, and limited conditional composition. The check uses the TypeScript syntax tree and ignores comments; it is not general cross-file data-flow analysis. Dynamic cross-file composition may be missed. Passing checks do not establish translation quality or complete runtime coverage; review the actual interface and language switching under the rules above.

`corepack pnpm test:repo` exercises checker failure cases; module tests cover language precedence, runtime switching, presentation formatting; end-to-end tests after a build confirm that language does not alter shared core data. This delivery covers Chinese and English CLI presentation. It does not add arbitrary language plugins, a general grammar expression language, right-to-left layout, a documentation website, or full translation of core diagnostics.
