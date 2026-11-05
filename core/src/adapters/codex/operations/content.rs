//! Locators for explicitly requested tool evidence, without retaining arguments or output.
use super::*;
use crate::conversation_source::{MAX_CHUNKS, OriginalContent};

pub(super) fn capture(item: &Payload<'_>, completed: bool, row: (usize, usize)) -> OriginalContent {
    let mut content = OriginalContent::default();
    let request = matches!(
        item.kind.as_deref(),
        Some("function_call" | "custom_tool_call")
    );
    let values = if request {
        [item.arguments.or(item.input), None, None]
    } else if completed {
        [item.command, item.aggregated_output, item.output]
    } else {
        [item.command, None, None]
    };
    for (index, raw) in values
        .into_iter()
        .enumerate()
        .filter_map(|(index, raw)| raw.map(|raw| (index, raw)))
    {
        let next = if request || index == 0 {
            request_content(raw, row)
        } else {
            timing::tool_result_content(raw, row)
        };
        let remaining = MAX_CHUNKS.saturating_sub(content.chunks.len());
        let part = content
            .chunks
            .last()
            .map_or(0, |chunk| chunk.part.saturating_add(1));
        content.partial |= next.partial || next.chunks.len() > remaining;
        content
            .chunks
            .extend(next.chunks.into_iter().take(remaining).map(|mut chunk| {
                chunk.part = chunk.part.saturating_add(part);
                chunk
            }));
    }
    content.chunks.sort_unstable_by_key(|chunk| chunk.offset);
    content.chunks.dedup_by_key(|chunk| chunk.offset);
    content
}
fn request_content(raw: &serde_json::value::RawValue, row: (usize, usize)) -> OriginalContent {
    let mut content = OriginalContent::default();
    if raw.get().starts_with('"') {
        content.text(raw, row, 0);
    } else if raw.get().starts_with('[') {
        struct Parts<'a>(&'a mut OriginalContent, (usize, usize));
        impl<'de> serde::de::Visitor<'de> for Parts<'_> {
            type Value = ();
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("command string arguments")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<(), A::Error> {
                let mut part = 0;
                while let Some(raw) = sequence.next_element::<&serde_json::value::RawValue>()? {
                    if part >= MAX_CHUNKS || self.0.chunks.len() >= MAX_CHUNKS {
                        return Err(serde::de::Error::custom("request locator limit"));
                    }
                    self.0.text(raw, self.1, part as u32);
                    part += 1;
                }
                Ok(())
            }
        }
        use serde::Deserializer;
        if serde_json::Deserializer::from_str(raw.get())
            .deserialize_seq(Parts(&mut content, row))
            .is_err()
        {
            content.partial = true;
        }
    } else {
        content.partial = true;
    }
    content
}
