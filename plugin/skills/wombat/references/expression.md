# Answer rules

Apply these writing principles from [ASD-STE100 Issue 9](https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf).

- Use direct verbs and active voice. Give each instruction one action, unless actions must occur together.
- Put a necessary condition before its action. Keep exceptions and recovery steps.
- In English, limit instruction sentences to 20 words and descriptive sentences to 25 words. Use the standard's word-count rules.
- Give each sentence one main idea and each paragraph one topic. Limit English paragraphs to six sentences.
- Use one term for each concept. Use the object name again when a pronoun can have more than one meaning.
- Use familiar words and established technical terms. Do not force dictionary substitutions that change meaning or make the answer unnatural.
- Do not use contractions or semicolons in English prose. Keep commands and source quotations unchanged.

Use STE writing principles as guidance, without strict dictionary matching. In Chinese, use short sentences and natural Chinese grammar. Do not apply English word counts to Chinese.

Use human-readable dates and durations. If the user requests raw evidence or debugging detail, show raw timestamps. For a date range, identify inclusive and exclusive boundaries. Do not describe an exclusive end as a recorded date.

Leave internal fields and verification steps out of simple answers unless they change the result or its reliability.

Before sending, make sure that the answer keeps the evidence, scope, units, uncertainty and authorization.

For a simple query, give the result and scope first. Add only limits that affect the result. For an analysis, give the conclusion, measured evidence and next action. Do not expose internal fields without a user need.

Example with synthetic facts:

> The known subtotal is USD 7.25. Another 1,000 Tokens have no price. The total cost is incomplete. This API-equivalent estimate is not a subscription charge.

> 已知部分的估算费用为 7.25 美元。另有 1,000 Token 无法计价，因此总费用不完整。这是 API 等价估算，不是订阅账单。
