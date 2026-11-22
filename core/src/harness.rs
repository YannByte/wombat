use crate::{dto::operation_error, hash, storage};
use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

const FORMAT: u8 = 1;
const MAX_TEXT: usize = 16 * 1024;
const MAX_FILE: usize = 256 * 1024;
const MAX_TOTAL: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    /// Freeze project files and checks for explicit review before starting model calls.
    Plan {
        project: String,
        task: String,
        files: Vec<String>,
        checks: Vec<Check>,
        #[serde(default)]
        include_global_instructions: bool,
    },
    Run {
        trial_id: String,
    },
    Status {
        trial_id: String,
    },
    Apply {
        trial_id: String,
        revision: String,
    },
    Verify {
        trial_id: String,
    },
    Undo {
        trial_id: String,
        revision: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Check {
    /// Exact observed shell invocation. Wombat compares evidence and never runs this string.
    Command { id: String, command: String },
    /// Exact content of a named output within the isolated workspace.
    File {
        id: String,
        path: String,
        equals: String,
    },
}

impl Check {
    fn id(&self) -> &str {
        match self {
            Self::Command { id, .. } | Self::File { id, .. } => id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    project: String,
    task: String,
    target: String,
    original_sha256: String,
    inputs: Vec<Input>,
    checks: Vec<Check>,
    include_global_instructions: bool,
    global_instruction_sources: Vec<Input>,
    executions: u8,
    editorial_sessions: u8,
    turn_timeout_seconds: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Planned,
    Running,
    Incomplete,
    Complete,
    Applying,
    Applied,
    Undoing,
    Undone,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observation {
    id: String,
    passed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stage {
    stage: String,
    checks: Vec<Observation>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Outcome {
    model: String,
    stages: Vec<Stage>,
    candidate: String,
    rationale: String,
    report: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Recommendation {
    ReviewComposed,
    KeepBaseline,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    #[schemars(range(min = 1, max = 1))]
    output_version: u8,
    trial_id: String,
    directory: String,
    phase: Phase,
    plan: Plan,
    original: String,
    outcome: Option<Outcome>,
    recommendation: Option<Recommendation>,
    revision: Option<String>,
    loaded: bool,
    failure: Option<String>,
    failed_stage: Option<String>,
}

#[derive(Deserialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HostRequest {
    Plan {
        project: String,
        task: String,
        files: Vec<String>,
        checks: Vec<Check>,
        include_global_instructions: bool,
        global_instruction_sources: Vec<Input>,
    },
    Begin {
        trial_id: String,
    },
    Status {
        trial_id: String,
    },
    Finish {
        trial_id: String,
        outcome: Outcome,
    },
    Fail {
        trial_id: String,
        code: String,
        stage: Option<String>,
    },
    Apply {
        trial_id: String,
        revision: String,
    },
    Loaded {
        trial_id: String,
        revision: String,
    },
    Undo {
        trial_id: String,
        revision: String,
    },
}

fn invalid(message: &'static str) -> anyhow::Error {
    operation_error("INVALID_ARGUMENT", message)
}

fn conflict() -> anyhow::Error {
    operation_error(
        "TRIAL_CONFLICT",
        "The trial or project file changed. Review its current state.",
    )
}

fn bounded(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.contains('\0')
}

fn relative(value: &str) -> Result<&Path> {
    let path = Path::new(value);
    if value.len() > 240
        || value.contains('\\')
        || value.split('/').any(|part| {
            part.is_empty()
                || part.starts_with('.')
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
        || !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(invalid(
            "Select ordinary named files relative to the project.",
        ));
    }
    Ok(path)
}

fn read_regular(file: &Path, limit: usize) -> Result<String> {
    let info = fs::symlink_metadata(file).map_err(|_| conflict())?;
    if !info.is_file() || info.len() > limit as u64 || dunce::canonicalize(file)? != file {
        return Err(conflict());
    }
    #[cfg(unix)]
    if std::os::unix::fs::MetadataExt::nlink(&info) != 1 {
        return Err(conflict());
    }
    let mut contents = String::new();
    File::open(file)?
        .take(limit as u64 + 1)
        .read_to_string(&mut contents)?;
    if contents.len() > limit {
        return Err(conflict());
    }
    Ok(contents)
}

fn root() -> Result<PathBuf> {
    let dir = storage::data_home()?.join("harness-v1");
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(&dir)?;
    if fs::symlink_metadata(&dir)?.file_type().is_symlink() {
        return Err(conflict());
    }
    Ok(dunce::canonicalize(dir)?)
}

fn dir(trial_id: &str) -> Result<PathBuf> {
    let id = uuid::Uuid::parse_str(trial_id).map_err(|_| invalid("Invalid trial identifier."))?;
    if id.to_string() != trial_id {
        return Err(invalid("Invalid trial identifier."));
    }
    let dir = root()?.join(trial_id);
    if !fs::symlink_metadata(&dir).is_ok_and(|info| info.is_dir()) {
        return Err(operation_error(
            "TRIAL_NOT_FOUND",
            "The trial could not be found.",
        ));
    }
    Ok(dir)
}

fn save(dir: &Path, state: &Response) -> Result<()> {
    storage::atomic_write(&dir.join("state.json"), &serde_json::to_vec(state)?)
}

fn plan(
    project: String,
    task: String,
    files: Vec<String>,
    checks: Vec<Check>,
    include_global_instructions: bool,
    global_instruction_sources: Vec<Input>,
) -> Result<Response> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err(operation_error(
            "TRIAL_PROBE_PLATFORM",
            "Harness trials require an Apple silicon Mac.",
        ));
    }
    if !Path::new(&project).is_absolute()
        || !bounded(&task, MAX_TEXT)
        || files.is_empty()
        || files.len() > 24
        || checks.is_empty()
        || checks.len() > 16
    {
        return Err(invalid(
            "Select a project, a task, its files, and independent checks.",
        ));
    }
    let project = dunce::canonicalize(project)?;
    let target = project.join("AGENTS.md");
    if project.join("AGENTS.override.md").exists() {
        return Err(invalid(
            "This trial supports a project whose root instructions are AGENTS.md.",
        ));
    }
    let original = read_regular(&target, MAX_TEXT)?;
    let mut total = 0;
    let mut names = std::collections::HashSet::new();
    let mut inputs = Vec::with_capacity(files.len());
    for path in files {
        let local = relative(&path)?;
        if !names.insert(path.to_ascii_lowercase())
            || local.file_name().is_some_and(|name| {
                name.eq_ignore_ascii_case("AGENTS.md")
                    || name.eq_ignore_ascii_case("AGENTS.override.md")
            })
            || local == Path::new("deliverable.md")
        {
            return Err(invalid(
                "Select distinct task materials apart from instruction files and deliverable.md.",
            ));
        }
        let contents = read_regular(&project.join(local), MAX_FILE)?;
        total += contents.len();
        if total > MAX_TOTAL {
            return Err(invalid(
                "Selected task materials exceed the trial input limit.",
            ));
        }
        inputs.push(Input {
            path,
            sha256: hash(contents),
        });
    }
    names.clear();
    let mut check_bytes = 0;
    for check in &checks {
        if !bounded(check.id(), 96) || !names.insert(check.id().to_owned()) {
            return Err(invalid("Each check needs a distinct identifier."));
        }
        check_bytes += match check {
            Check::Command { command, .. } => command.len(),
            Check::File { equals, .. } => equals.len(),
        };
        if check_bytes > 128 * 1024 {
            return Err(invalid("The checks exceed the trial limit."));
        }
        match check {
            Check::Command { command, .. }
                if !bounded(command, 4096) || command.contains(['\n', '\r']) =>
            {
                return Err(invalid("Select one exact shell invocation to observe."));
            }
            Check::File { path, equals, .. } => {
                relative(path)?;
                if equals.len() > MAX_FILE
                    || inputs
                        .iter()
                        .any(|input| input.path.eq_ignore_ascii_case(path))
                {
                    return Err(invalid(
                        "File checks must describe an output produced by the task.",
                    ));
                }
            }
            _ => {}
        }
    }
    let trial_id = uuid::Uuid::new_v4().to_string();
    let data = root()?;
    if data.starts_with(&project) {
        return Err(invalid(
            "The Wombat trial directory must be outside the project.",
        ));
    }
    let root = data.join(&trial_id);
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(&root)?;
    let state = Response {
        output_version: FORMAT,
        trial_id,
        directory: root.to_string_lossy().into_owned(),
        phase: Phase::Planned,
        plan: Plan {
            project: project.to_string_lossy().into_owned(),
            task,
            target: target.to_string_lossy().into_owned(),
            original_sha256: hash(&original),
            inputs,
            checks,
            include_global_instructions,
            global_instruction_sources,
            executions: 4,
            editorial_sessions: 2,
            turn_timeout_seconds: 300,
        },
        original,
        outcome: None,
        recommendation: None,
        revision: None,
        loaded: false,
        failure: None,
        failed_stage: None,
    };
    save(&root, &state)?;
    Ok(state)
}

fn verify_hash(path: &Path, expected: &str) -> Result<()> {
    if hash(read_regular(path, MAX_FILE)?) != expected {
        return Err(conflict());
    }
    Ok(())
}

fn checked_candidate(state: &Response, revision: &str) -> Result<String> {
    let candidate = &state.outcome.as_ref().ok_or_else(conflict)?.candidate;
    if state.revision.as_deref() != Some(revision) || hash(candidate) != revision {
        return Err(conflict());
    }
    Ok(candidate.clone())
}

fn replace(path: &Path, expected: &str, content: &str) -> Result<()> {
    use std::io::Write;
    verify_hash(path, expected)?;
    let parent = path.parent().ok_or_else(conflict)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.as_file()
        .set_permissions(fs::metadata(path)?.permissions())?;
    file.write_all(content.as_bytes())?;
    file.as_file().sync_all()?;
    verify_hash(path, expected)?;
    file.persist(path)?;
    Ok(())
}

fn finish(state: &mut Response, outcome: Outcome) -> Result<()> {
    if state.phase != Phase::Running
        || !bounded(&outcome.candidate, MAX_TEXT)
        || !bounded(&outcome.rationale, MAX_TEXT)
        || !bounded(&outcome.model, 96)
        || outcome.stages.len() != 4
    {
        return Err(conflict());
    }
    for (stage, name) in outcome
        .stages
        .iter()
        .zip(["baseline", "general", "domain", "composed"])
    {
        if stage.stage != name
            || stage.checks.len() != state.plan.checks.len()
            || stage
                .checks
                .iter()
                .zip(&state.plan.checks)
                .any(|(report, check)| report.id != check.id())
        {
            return Err(conflict());
        }
    }
    let before = &outcome.stages[0].checks;
    let after = &outcome.stages[3].checks;
    let improves = before
        .iter()
        .zip(after)
        .any(|(old, new)| !old.passed && new.passed);
    let regresses = before
        .iter()
        .zip(after)
        .any(|(old, new)| old.passed && !new.passed);
    let revision = hash(&outcome.candidate);
    state.recommendation = Some(
        if improves && !regresses && revision != state.plan.original_sha256 {
            Recommendation::ReviewComposed
        } else {
            Recommendation::KeepBaseline
        },
    );
    state.revision = Some(revision);
    state.outcome = Some(outcome);
    state.phase = Phase::Complete;
    Ok(())
}

pub fn dispatch(request: HostRequest) -> Result<Response> {
    let trial_id = match &request {
        HostRequest::Plan { .. } => {
            let HostRequest::Plan {
                project,
                task,
                files,
                checks,
                include_global_instructions,
                global_instruction_sources,
            } = request
            else {
                unreachable!()
            };
            return plan(
                project,
                task,
                files,
                checks,
                include_global_instructions,
                global_instruction_sources,
            );
        }
        HostRequest::Begin { trial_id }
        | HostRequest::Status { trial_id }
        | HostRequest::Finish { trial_id, .. }
        | HostRequest::Fail { trial_id, .. }
        | HostRequest::Apply { trial_id, .. }
        | HostRequest::Loaded { trial_id, .. }
        | HostRequest::Undo { trial_id, .. } => trial_id,
    };
    let dir = dir(trial_id)?;
    let mutex = dir.join("lease");
    storage::private_file(&mutex)?;
    let lock = fs::OpenOptions::new().read(true).write(true).open(mutex)?;
    lock.try_lock()
        .map_err(|_| operation_error("TRIAL_BUSY", "The trial is being updated."))?;
    let mut state: Response =
        serde_json::from_str(&read_regular(&dir.join("state.json"), 512 * 1024)?)?;
    if state.output_version != FORMAT || state.trial_id != *trial_id {
        return Err(conflict());
    }
    let target = PathBuf::from(&state.plan.target);
    if matches!(state.phase, Phase::Applying | Phase::Undoing) {
        let revision = state.revision.as_deref().ok_or_else(conflict)?;
        let at_candidate = verify_hash(&target, revision).is_ok();
        if !at_candidate {
            verify_hash(&target, &state.plan.original_sha256)?;
        }
        state.phase = match (state.phase, at_candidate) {
            (_, true) => Phase::Applied,
            (Phase::Applying, false) => Phase::Complete,
            _ => Phase::Undone,
        };
        state.loaded = false;
        save(&dir, &state)?;
    }
    match request {
        HostRequest::Status { .. } => {
            if let Some(revision) = &state.revision
                && state.phase == Phase::Applied
                && verify_hash(&target, revision).is_err()
            {
                state.loaded = false;
            }
            return Ok(state);
        }
        HostRequest::Begin { .. } if state.phase == Phase::Planned => {
            if target.with_file_name("AGENTS.override.md").exists() {
                return Err(conflict());
            }
            verify_hash(&target, &state.plan.original_sha256)?;
            let project = Path::new(&state.plan.project);
            for input in &state.plan.inputs {
                verify_hash(&project.join(relative(&input.path)?), &input.sha256)?;
            }
            state.phase = Phase::Running;
        }
        HostRequest::Finish { outcome, .. } => {
            let report = Path::new(&outcome.report);
            if !report.starts_with(&dir)
                || report.file_name().and_then(|p| p.to_str()) != Some("report.json")
                || !fs::symlink_metadata(report).is_ok_and(|info| info.is_file())
                || dunce::canonicalize(report)? != report
            {
                return Err(conflict());
            }
            finish(&mut state, outcome)?;
        }
        HostRequest::Fail { code, stage, .. } if state.phase == Phase::Running => {
            state.phase = Phase::Incomplete;
            state.failed_stage = stage.filter(|value| {
                matches!(
                    value.as_str(),
                    "host" | "baseline" | "propose" | "general" | "domain" | "compose" | "composed"
                )
            });
            state.failure = Some(
                code.chars()
                    .filter(|c| c.is_ascii_uppercase() || *c == '_')
                    .take(60)
                    .collect(),
            );
        }
        HostRequest::Apply { revision, .. }
            if state.phase == Phase::Complete
                && matches!(state.recommendation, Some(Recommendation::ReviewComposed)) =>
        {
            let candidate = checked_candidate(&state, &revision)?;
            if target.with_file_name("AGENTS.override.md").exists() {
                return Err(conflict());
            }
            let project = Path::new(&state.plan.project);
            for input in &state.plan.inputs {
                verify_hash(&project.join(relative(&input.path)?), &input.sha256)?;
            }
            state.phase = Phase::Applying;
            save(&dir, &state)?;
            replace(&target, &state.plan.original_sha256, &candidate)?;
            state.phase = Phase::Applied;
        }
        HostRequest::Loaded { revision, .. } if state.phase == Phase::Applied => {
            checked_candidate(&state, &revision)?;
            verify_hash(&target, &revision)?;
            state.loaded = true;
        }
        HostRequest::Undo { revision, .. } if state.phase == Phase::Applied => {
            checked_candidate(&state, &revision)?;
            state.phase = Phase::Undoing;
            save(&dir, &state)?;
            replace(&target, &revision, &state.original)?;
            state.phase = Phase::Undone;
            state.loaded = false;
        }
        _ => return Err(conflict()),
    }
    save(&dir, &state)?;
    Ok(state)
}
