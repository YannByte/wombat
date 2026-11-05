//! Determinate command requests and verified native read targets; source bodies stay in memory.
//! A request digest never identifies the installed executable, environment or file contents.
use super::*;
use crate::adapters::contract::{
    MATCH_STRING_BYTES, MATCH_TARGET_LIMIT, MatchGap, OPERATION_MATCH_VERSION,
    OperationMatchObservation, ReadMatchTarget, SourcePathPlatform,
};
use serde::de::{SeqAccess, Visitor};

// Validate the native item's closed request shape without allocating outputs or scripts.
struct ShapeVisitor;
impl<'de> Visitor<'de> for ShapeVisitor {
    type Value = bool;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("supported native command item")
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<bool, A::Error> {
        let mut supported = true;
        while let Some(key) = map.next_key::<&str>()? {
            let raw = map.next_value::<&RawValue>()?;
            supported &= matches!(
                key,
                "type"
                    | "id"
                    | "item_id"
                    | "itemId"
                    | "call_id"
                    | "callId"
                    | "thread_id"
                    | "turn_id"
                    | "response_id"
                    | "command"
                    | "cwd"
                    | "parsed_cmd"
                    | "source"
                    | "status"
                    | "stdout"
                    | "stderr"
                    | "aggregated_output"
                    | "output"
                    | "exit_code"
                    | "exitCode"
                    | "duration"
                    | "duration_ms"
                    | "durationMs"
                    | "started_at_ms"
                    | "completed_at_ms"
                    | "formatted_output"
                    | "process_id"
                    | "plugin_id"
                    | "script_path"
                    | "interaction_input"
            );
            // These optional fields can add execution inputs not represented by argv/cwd.
            if matches!(key, "plugin_id" | "script_path" | "interaction_input")
                && raw.get() != "null"
            {
                supported = false;
            }
        }
        Ok(supported)
    }
}
struct Shape(bool);
impl<'de> Deserialize<'de> for Shape {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(ShapeVisitor).map(Self)
    }
}
pub(super) fn command_shape(raw: Option<&RawValue>) -> bool {
    raw.and_then(|raw| serde_json::from_str::<Shape>(raw.get()).ok())
        .is_some_and(|shape| shape.0)
}

#[derive(Default)]
struct Args {
    values: Vec<String>,
    invalid: bool,
    limited: bool,
}
struct ArgsVisitor;
impl<'de> Visitor<'de> for ArgsVisitor {
    type Value = Args;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("bounded native command argv")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Args, A::Error> {
        let mut result = Args::default();
        let mut bytes = 0usize;
        while let Some(raw) = seq.next_element::<&RawValue>()? {
            if result.invalid || result.limited {
                continue;
            }
            bytes = bytes.saturating_add(raw.get().len());
            if bytes > MATCH_STRING_BYTES || result.values.len() >= 1024 {
                result.limited = true;
                result.values.clear();
                continue;
            }
            match serde_json::from_str::<String>(raw.get()) {
                Ok(value) if !value.contains('\0') => result.values.push(value),
                _ => {
                    result.invalid = true;
                    result.values.clear();
                }
            }
        }
        Ok(result)
    }
}
impl<'de> Deserialize<'de> for Args {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_seq(ArgsVisitor)
    }
}
fn add(gaps: &mut Vec<MatchGap>, gap: MatchGap) {
    if !gaps.contains(&gap) {
        gaps.push(gap);
    }
}
fn path_text(value: &str) -> Option<String> {
    let raw = if let Some(uri) = value.strip_prefix("file://") {
        let raw = uri.strip_prefix("localhost").unwrap_or(uri);
        if !raw.starts_with('/') || raw.contains(['?', '#']) {
            return None;
        }
        let decoded = percent_encoding::percent_decode_str(raw)
            .decode_utf8()
            .ok()?;
        if decoded.as_bytes().get(2) == Some(&b':')
            && decoded
                .as_bytes()
                .get(1)
                .is_some_and(u8::is_ascii_alphabetic)
        {
            decoded[1..].to_owned()
        } else {
            decoded.into_owned()
        }
    } else {
        if value.contains("://") {
            return None;
        }
        value.to_owned()
    };
    valid_work_path(&raw).then_some(raw)
}
fn platform(path: &str) -> Option<SourcePathPlatform> {
    let bytes = path.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
        || path.starts_with("\\\\")
    {
        Some(SourcePathPlatform::Windows)
    } else if path.starts_with('/') {
        Some(SourcePathPlatform::Posix)
    } else {
        None
    }
}
fn normalize(path: &str, platform: &SourcePathPlatform) -> Option<String> {
    let path = if *platform == SourcePathPlatform::Windows {
        path.replace('\\', "/")
    } else {
        path.to_owned()
    };
    // Parent traversal can depend on historical symlinks. Keep it unlocated instead of guessing.
    if path.split('/').any(|part| part == "..") {
        return None;
    }
    let prefix = if path.starts_with("//") && !path.starts_with("///") {
        "//"
    } else if path.starts_with('/') {
        "/"
    } else {
        ""
    };
    let parts = path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Some("/".into());
    }
    if *platform == SourcePathPlatform::Windows && prefix == "//" && parts.len() < 2 {
        return None;
    }
    let mut out = format!("{prefix}{}", parts.join("/"));
    if *platform == SourcePathPlatform::Windows && out.as_bytes().get(1) == Some(&b':') {
        out.replace_range(..1, &out[..1].to_ascii_lowercase());
    }
    if *platform == SourcePathPlatform::Windows && out.len() == 2 && out.ends_with(':') {
        out.push('/');
    }
    Some(out)
}
pub(crate) fn target(cwd: &str, path: &str) -> Option<ReadMatchTarget> {
    let cwd = path_text(cwd)?;
    let path = path_text(path)?;
    let owner = platform(&cwd)?;
    let absolute = platform(&path);
    if absolute.as_ref().is_some_and(|kind| *kind != owner) {
        return None;
    }
    if owner == SourcePathPlatform::Windows
        && absolute.is_none()
        && (path.starts_with(['/', '\\']) || path.as_bytes().get(1) == Some(&b':'))
    {
        return None;
    }
    let joined = if absolute.is_some() {
        path
    } else {
        format!("{cwd}/{path}")
    };
    Some(ReadMatchTarget {
        path: normalize(&joined, &owner)?,
        platform: owner,
    })
}
fn basename(value: &str) -> &str {
    value.rsplit(['/', '\\']).next().unwrap_or(value)
}
fn interpreter_name(value: &str) -> String {
    let name = basename(value).to_ascii_lowercase();
    name.strip_suffix(".exe").unwrap_or(&name).to_owned()
}
fn literal_invocation(argv: &[String]) -> Option<Vec<String>> {
    let first = argv.first()?;
    if first.is_empty() {
        return None;
    }
    // Exclude interpreter aliases conservatively, including Windows case and .exe spelling.
    match interpreter_name(first).as_str() {
        "sh" | "bash" | "zsh" => {
            if argv.len() != 3
                || !matches!(argv[1].as_str(), "-c" | "-lc")
                || argv[2].is_empty()
                || argv[2].contains([
                    '$', '`', ';', '|', '&', '<', '>', '\n', '\r', '*', '?', '[', ']', '~', '(',
                    ')', '{', '}',
                ])
            {
                return None;
            }
            let words = shlex::split(&argv[2])?;
            if words.first().is_none_or(|v| {
                v.is_empty()
                    || v.contains('=')
                    || matches!(
                        interpreter_name(v).as_str(),
                        "eval"
                            | "exec"
                            | "source"
                            | "."
                            | "env"
                            | "sh"
                            | "bash"
                            | "zsh"
                            | "cmd"
                            | "powershell"
                            | "pwsh"
                            | "fish"
                            | "if"
                            | "then"
                            | "else"
                            | "elif"
                            | "fi"
                            | "for"
                            | "while"
                            | "until"
                            | "do"
                            | "done"
                            | "case"
                            | "esac"
                            | "function"
                    )
            }) {
                return None;
            }
            Some(words)
        }
        "cmd" | "powershell" | "pwsh" | "fish" | "env" | "eval" => None,
        _ => Some(argv.to_vec()),
    }
}
fn read_paths(argv: &[String]) -> Option<Vec<&str>> {
    let first = basename(argv.first()?);
    let mut paths = vec![];
    let mut index = 1;
    let mut options = true;
    match first {
        "cat" => {
            while index < argv.len() {
                let arg = argv[index].as_str();
                index += 1;
                if options && arg == "--" {
                    options = false;
                    continue;
                }
                if options && arg.starts_with('-') {
                    if !matches!(arg, "-n" | "-b" | "-E" | "-T" | "-s" | "-v" | "-A") {
                        return None;
                    }
                    continue;
                }
                if arg == "-" || arg.is_empty() {
                    return None;
                }
                paths.push(arg);
            }
        }
        "head" | "tail" => {
            while index < argv.len() {
                let arg = argv[index].as_str();
                index += 1;
                if options && arg == "--" {
                    options = false;
                    continue;
                }
                if options && arg.starts_with('-') {
                    if matches!(arg, "-n" | "-c") {
                        let n = argv.get(index)?.parse::<u64>().ok()?;
                        index += 1;
                        if n == 0 {
                            return None;
                        }
                    } else {
                        return None;
                    }
                    continue;
                }
                if arg == "-" || arg.is_empty() {
                    return None;
                }
                paths.push(arg);
            }
        }
        "sed" => {
            if argv.len() != 4 || argv[1] != "-n" {
                return None;
            }
            let range = argv[2].strip_suffix('p')?;
            let (start, end) = range.split_once(',').unwrap_or((range, range));
            let start = start.parse::<u64>().ok()?;
            let end = end.parse::<u64>().ok()?;
            if start == 0 || end < start || argv[3].starts_with('-') {
                return None;
            }
            paths.push(argv[3].as_str());
        }
        _ => return None,
    }
    (!paths.is_empty()).then_some(paths)
}
pub(super) fn observe(
    raw: Option<&RawValue>,
    receiver: Option<&str>,
    source: Option<&CommandSource>,
    cwd: Option<&str>,
    parsed: Option<&[ParsedCommand]>,
) -> OperationMatchObservation {
    let mut out = OperationMatchObservation {
        format_version: OPERATION_MATCH_VERSION,
        receiver_owner: None,
        request_fingerprint: None,
        function_request_fingerprint: None,
        read_targets: vec![],
        expected_nonzero: false,
        gaps: vec![],
    };
    if matches!(
        source,
        Some(CommandSource::Agent | CommandSource::UnifiedExecStartup)
    ) {
        out.receiver_owner = receiver.filter(|v| !v.is_empty()).map(str::to_owned);
    }
    if out.receiver_owner.is_none() {
        add(&mut out.gaps, MatchGap::MissingReceiver);
    }
    let Some(cwd) = cwd.filter(|cwd| target(cwd, ".").is_some()) else {
        add(&mut out.gaps, MatchGap::MissingHistoricalCwd);
        return out;
    };
    let Some(raw) = raw else {
        add(&mut out.gaps, MatchGap::MissingParameters);
        return out;
    };
    let args = match serde_json::from_str::<Args>(raw.get()) {
        Ok(value) => value,
        Err(_) => {
            add(&mut out.gaps, MatchGap::UnsupportedParameters);
            return out;
        }
    };
    if args.limited {
        add(&mut out.gaps, MatchGap::ResourceLimit);
        return out;
    }
    if args.invalid || args.values.first().is_none_or(String::is_empty) {
        add(&mut out.gaps, MatchGap::UnsupportedParameters);
        return out;
    }
    // Full original argv, including a literal shell request, remains part of the digest.
    // Equal digests do not establish equal binary versions, PATH, input files or effects.
    out.request_fingerprint = Some(crate::hash(
        serde_json::to_vec(&("native_command_request_v1", cwd, &args.values)).unwrap(),
    ));
    // Equality of the recorded argv does not require interpreting a script.
    // Only read-target and expected-exit classification require literal parsing.
    let Some(invocation) = literal_invocation(&args.values) else {
        if parsed.is_some_and(|p| p.iter().any(|c| matches!(c, ParsedCommand::Read { .. }))) {
            add(&mut out.gaps, MatchGap::UnconfirmedRead);
        }
        return out;
    };
    out.expected_nonzero = matches!(
        basename(&invocation[0]),
        "rg" | "grep" | "egrep" | "fgrep" | "diff" | "cmp" | "test" | "["
    );
    let Some(paths) = read_paths(&invocation) else {
        if parsed.is_some_and(|p| p.iter().any(|c| matches!(c, ParsedCommand::Read { .. }))) {
            add(&mut out.gaps, MatchGap::UnconfirmedRead);
        }
        return out;
    };
    let mut requested = BTreeSet::new();
    let mut requested_bytes = 0usize;
    for path in paths {
        let Some(value) = target(cwd, path) else {
            add(&mut out.gaps, MatchGap::UnresolvedReadTarget);
            continue;
        };
        if requested.insert(value.clone()) {
            requested_bytes = requested_bytes.saturating_add(value.path.len());
        }
        if requested_bytes > MATCH_STRING_BYTES {
            add(&mut out.gaps, MatchGap::ResourceLimit);
            return out;
        }
    }
    let Some(parsed) = parsed else {
        add(&mut out.gaps, MatchGap::UnconfirmedRead);
        return out;
    };
    let mut found = BTreeSet::new();
    let mut found_bytes = 0usize;
    for command in parsed {
        let ParsedCommand::Read { path } = command else {
            continue;
        };
        let Some(candidate) = path.as_deref().and_then(|p| target(cwd, p)) else {
            add(&mut out.gaps, MatchGap::UnresolvedReadTarget);
            continue;
        };
        if requested.contains(&candidate) {
            if found.insert(candidate.clone()) {
                found_bytes = found_bytes.saturating_add(candidate.path.len());
            }
            if found.len() > MATCH_TARGET_LIMIT || found_bytes > MATCH_STRING_BYTES {
                add(&mut out.gaps, MatchGap::ResourceLimit);
                return out;
            }
        } else {
            add(&mut out.gaps, MatchGap::UnconfirmedRead);
        }
    }
    if found.is_empty() {
        add(&mut out.gaps, MatchGap::UnconfirmedRead);
    }
    out.read_targets = found.into_iter().collect();
    out
}
#[cfg(test)]
use super::super::matching::merge;
#[cfg(test)]
mod tests;
