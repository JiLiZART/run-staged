//! Pure view model: folds timestamped events into a tree and computes the rows
//! to draw at an instant `now`. No terminal, no clock — fully testable.

use std::time::{Duration, Instant};

use run_staged_exec::{CommandOutcome, ExecEvent, PlannedInvocation};
use run_staged_output::{ansi, format_duration};

/// Spinner frames, advanced every [`SPIN_FRAME`].
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub const SPIN_FRAME: Duration = Duration::from_millis(80);
/// One full pulse of a running row's color (6 steps).
pub const PULSE_PERIOD: Duration = Duration::from_millis(1200);
pub const PULSE_STEPS: usize = 6;
/// How long a step label takes to type itself in.
pub const TYPE_IN: Duration = Duration::from_millis(150);
/// How long a finished row stays emphasized.
pub const HIGHLIGHT: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Backup,
    Tasks,
    Stage,
    Cleanup,
}

/// A group in the plan, as the binary hands it over from `run_staged_core::Plan`.
#[derive(Debug, Clone)]
pub struct PlanGroup {
    pub name: String,
    pub patterns: Vec<String>,
    pub file_count: usize,
}

#[derive(Debug, Clone)]
pub struct PlanInfo {
    pub unstaged: bool,
    pub file_count: usize,
    pub groups: Vec<PlanGroup>,
    pub invocations: Vec<PlannedInvocation>,
}

/// Everything the live view reacts to.
#[derive(Debug, Clone)]
pub enum Event {
    Plan(PlanInfo),
    StepStarted(StepKind),
    StepFinished(StepKind),
    StepFailed(StepKind),
    Exec(ExecEvent),
    /// The run is over: draw the final frame.
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
struct StepRow {
    kind: StepKind,
    phase: Phase,
    started: Option<Instant>,
    finished: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdStatus {
    Queued,
    Running,
    Success,
    Failed,
    TimedOut,
    Cancelled,
}

impl CmdStatus {
    fn finished(self) -> bool {
        !matches!(self, CmdStatus::Queued | CmdStatus::Running)
    }
    /// Severity for a group's summary marker.
    fn rank(self) -> u8 {
        match self {
            CmdStatus::Failed => 5,
            CmdStatus::TimedOut => 4,
            CmdStatus::Cancelled => 3,
            CmdStatus::Running => 2,
            CmdStatus::Queued => 1,
            CmdStatus::Success => 0,
        }
    }
}

#[derive(Debug, Clone)]
struct CmdRow {
    id: usize,
    command: String,
    status: CmdStatus,
    started: Option<Instant>,
    finished: Option<Instant>,
    tail: Option<String>,
}

#[derive(Debug, Clone)]
struct GroupRow {
    name: String,
    patterns: String,
    file_count: usize,
    cmds: Vec<usize>, // indexes into Model::cmds
}

/// How a row is drawn; the renderer maps this to colors and modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Pending,
    /// Running; `pulse` is the brightness step `0..PULSE_STEPS`.
    Running {
        pulse: usize,
    },
    Done {
        emph: bool,
    },
    Failed {
        emph: bool,
    },
    TimedOut {
        emph: bool,
    },
    Cancelled,
    Group,
    Tail,
    More,
}

/// One line of the step tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub depth: u16,
    pub glyph: String,
    pub label: String,
    /// Right-hand detail such as a duration or file count.
    pub detail: String,
    pub tone: Tone,
}

/// Overall state, for the panel border and gauge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overall {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub struct Model {
    started: Option<Instant>,
    ended: Option<Instant>,
    unstaged: bool,
    file_count: usize,
    steps: Vec<StepRow>,
    groups: Vec<GroupRow>,
    cmds: Vec<CmdRow>,
}

impl Model {
    /// Applies an event stamped at `at`.
    pub fn apply(&mut self, event: Event, at: Instant) {
        self.started.get_or_insert(at);
        match event {
            Event::Plan(plan) => self.plan(plan),
            Event::StepStarted(k) => self.step(k, Phase::Running, at),
            Event::StepFinished(k) => self.step(k, Phase::Done, at),
            Event::StepFailed(k) => self.step(k, Phase::Failed, at),
            Event::Exec(e) => self.exec(e, at),
            Event::Done => {
                self.ended = Some(at);
                for c in &mut self.cmds {
                    c.tail = None;
                }
            }
        }
    }

    fn plan(&mut self, plan: PlanInfo) {
        self.unstaged = plan.unstaged;
        self.file_count = plan.file_count;
        let kinds: &[StepKind] = if plan.unstaged {
            &[StepKind::Tasks]
        } else {
            &[
                StepKind::Backup,
                StepKind::Tasks,
                StepKind::Stage,
                StepKind::Cleanup,
            ]
        };
        self.steps = kinds
            .iter()
            .map(|&kind| StepRow {
                kind,
                phase: Phase::Pending,
                started: None,
                finished: None,
            })
            .collect();
        self.cmds = plan
            .invocations
            .iter()
            .map(|inv| CmdRow {
                id: inv.id,
                command: inv.command.clone(),
                status: CmdStatus::Queued,
                started: None,
                finished: None,
                tail: None,
            })
            .collect();
        self.groups = plan
            .groups
            .into_iter()
            .map(|g| GroupRow {
                cmds: plan
                    .invocations
                    .iter()
                    .enumerate()
                    .filter(|(_, inv)| inv.group == g.name)
                    .map(|(i, _)| i)
                    .collect(),
                name: g.name,
                patterns: g.patterns.join(" "),
                file_count: g.file_count,
            })
            .collect();
    }

    fn step(&mut self, kind: StepKind, phase: Phase, at: Instant) {
        if let Some(s) = self.steps.iter_mut().find(|s| s.kind == kind) {
            s.phase = phase;
            match phase {
                Phase::Running => s.started = Some(at),
                _ => {
                    s.started.get_or_insert(at);
                    s.finished = Some(at);
                }
            }
        }
    }

    fn exec(&mut self, e: ExecEvent, at: Instant) {
        let id = match &e {
            ExecEvent::Started { id }
            | ExecEvent::Line { id, .. }
            | ExecEvent::Finished { id, .. } => *id,
        };
        let Some(c) = self.cmds.iter_mut().find(|c| c.id == id) else {
            return;
        };
        match e {
            ExecEvent::Started { .. } => {
                c.status = CmdStatus::Running;
                c.started = Some(at);
            }
            ExecEvent::Line { text, .. } => {
                if c.status == CmdStatus::Running {
                    let clean = ansi::sanitize(&text, ansi::Sgr::Strip);
                    if !clean.trim().is_empty() {
                        c.tail = Some(clean.trim_end().to_string());
                    }
                }
            }
            ExecEvent::Finished { outcome, .. } => {
                c.status = match outcome {
                    CommandOutcome::Success { .. } => CmdStatus::Success,
                    CommandOutcome::Failure {
                        timed_out: true, ..
                    } => CmdStatus::TimedOut,
                    CommandOutcome::Failure { .. } => CmdStatus::Failed,
                    CommandOutcome::Cancelled { .. } => CmdStatus::Cancelled,
                };
                c.started.get_or_insert(at);
                c.finished = Some(at);
                c.tail = None;
            }
        }
    }

    pub fn file_count(&self) -> usize {
        self.file_count
    }

    pub fn is_done(&self) -> bool {
        self.ended.is_some()
    }

    /// Time since the first event, frozen once the run is done.
    pub fn elapsed(&self, now: Instant) -> Duration {
        match self.started {
            Some(s) => self.ended.unwrap_or(now).saturating_duration_since(s),
            None => Duration::ZERO,
        }
    }

    /// `(finished, total)` invocations, for the gauge.
    pub fn progress(&self) -> (usize, usize) {
        let done = self.cmds.iter().filter(|c| c.status.finished()).count();
        (done, self.cmds.len())
    }

    pub fn overall(&self) -> Overall {
        let failed = self.steps.iter().any(|s| s.phase == Phase::Failed)
            || self.cmds.iter().any(|c| {
                matches!(
                    c.status,
                    CmdStatus::Failed | CmdStatus::TimedOut | CmdStatus::Cancelled
                )
            });
        if failed {
            Overall::Failed
        } else if self.is_done() {
            Overall::Succeeded
        } else {
            Overall::Running
        }
    }

    /// Rows that fit in `max_rows`, reduced in spec order: drop output tails,
    /// collapse finished groups, then replace what still does not fit with
    /// `… N more`.
    pub fn rows(&self, now: Instant, max_rows: usize) -> Vec<Row> {
        let full = self.layout(now, true, false);
        if full.len() <= max_rows {
            return full.into_iter().map(|(r, _)| r).collect();
        }
        let no_tails = self.layout(now, false, false);
        if no_tails.len() <= max_rows {
            return no_tails.into_iter().map(|(r, _)| r).collect();
        }
        let collapsed = self.layout(now, false, true);
        if collapsed.len() <= max_rows {
            return collapsed.into_iter().map(|(r, _)| r).collect();
        }
        truncate(collapsed, max_rows)
    }

    /// All rows with a keep-priority flag (started steps, running commands and their groups).
    fn layout(&self, now: Instant, tails: bool, collapse: bool) -> Vec<(Row, bool)> {
        let mut out = Vec::new();
        for s in &self.steps {
            // Steps that never ran are dropped from the final frame.
            if self.is_done() && s.phase == Phase::Pending {
                continue;
            }
            // Pending steps say little; they give way to running commands.
            out.push((self.step_row(s, now), s.phase != Phase::Pending));
            if s.kind == StepKind::Tasks {
                for g in &self.groups {
                    self.group_rows(g, now, tails, collapse, &mut out);
                }
            }
        }
        out
    }

    fn step_row(&self, s: &StepRow, now: Instant) -> Row {
        let (pending, running, done, failed) = labels(s.kind, self.unstaged);
        match s.phase {
            Phase::Pending => Row {
                depth: 0,
                glyph: "◌".into(),
                label: pending.into(),
                detail: String::new(),
                tone: Tone::Pending,
            },
            Phase::Running => {
                let since = now.saturating_duration_since(s.started.unwrap_or(now));
                Row {
                    depth: 0,
                    glyph: spinner(since).into(),
                    label: type_in(running, since),
                    detail: format_duration(since),
                    tone: Tone::Running {
                        pulse: pulse(since),
                    },
                }
            }
            Phase::Done | Phase::Failed => {
                let (started, finished) = (s.started.unwrap_or(now), s.finished.unwrap_or(now));
                let emph = emphasized(finished, now);
                let ok = s.phase == Phase::Done;
                Row {
                    depth: 0,
                    glyph: if ok { "✔" } else { "✗" }.into(),
                    label: if ok { done } else { failed }.into(),
                    detail: format_duration(finished.saturating_duration_since(started)),
                    tone: if ok {
                        Tone::Done { emph }
                    } else {
                        Tone::Failed { emph }
                    },
                }
            }
        }
    }

    fn group_rows(
        &self,
        g: &GroupRow,
        now: Instant,
        tails: bool,
        collapse: bool,
        out: &mut Vec<(Row, bool)>,
    ) {
        let cmds: Vec<&CmdRow> = g.cmds.iter().map(|&i| &self.cmds[i]).collect();
        let all_finished = !cmds.is_empty() && cmds.iter().all(|c| c.status.finished());
        let any_running = cmds.iter().any(|c| c.status == CmdStatus::Running);
        let files = if g.file_count == 1 { "file" } else { "files" };
        let (glyph, tone) = if all_finished {
            let worst = cmds
                .iter()
                .map(|c| c.status)
                .max_by_key(|s| s.rank())
                .unwrap();
            let last = cmds.iter().filter_map(|c| c.finished).max().unwrap_or(now);
            final_marker(worst, emphasized(last, now))
        } else {
            ("▸", Tone::Group)
        };
        out.push((
            Row {
                depth: 1,
                glyph: glyph.into(),
                label: format!("{}  {}", g.name, g.patterns),
                detail: format!("{} {files}", g.file_count),
                tone,
            },
            any_running,
        ));
        if collapse && all_finished {
            return;
        }
        for c in cmds {
            out.push((self.cmd_row(c, now), c.status == CmdStatus::Running));
            if tails && let Some(t) = &c.tail {
                out.push((
                    Row {
                        depth: 3,
                        glyph: "│".into(),
                        label: t.clone(),
                        detail: String::new(),
                        tone: Tone::Tail,
                    },
                    false,
                ));
            }
        }
    }

    fn cmd_row(&self, c: &CmdRow, now: Instant) -> Row {
        let (glyph, detail, tone) = match c.status {
            CmdStatus::Queued => ("◌".to_string(), String::new(), Tone::Pending),
            CmdStatus::Running => {
                let since = now.saturating_duration_since(c.started.unwrap_or(now));
                (
                    spinner(since).to_string(),
                    format_duration(since),
                    Tone::Running {
                        pulse: pulse(since),
                    },
                )
            }
            done => {
                let finished = c.finished.unwrap_or(now);
                let took = finished.saturating_duration_since(c.started.unwrap_or(finished));
                let (glyph, tone) = final_marker(done, emphasized(finished, now));
                (glyph.to_string(), format_duration(took), tone)
            }
        };
        Row {
            depth: 2,
            glyph,
            label: c.command.clone(),
            detail,
            tone,
        }
    }
}

fn final_marker(status: CmdStatus, emph: bool) -> (&'static str, Tone) {
    match status {
        CmdStatus::Success => ("✔", Tone::Done { emph }),
        CmdStatus::Failed => ("✗", Tone::Failed { emph }),
        CmdStatus::TimedOut => ("⏱", Tone::TimedOut { emph }),
        CmdStatus::Cancelled => ("⊘", Tone::Cancelled),
        CmdStatus::Queued => ("◌", Tone::Pending),
        CmdStatus::Running => ("▸", Tone::Group),
    }
}

/// Keeps priority rows (steps, running commands and their groups) and fills
/// the rest in order; whatever is dropped becomes one `… N more` row.
fn truncate(rows: Vec<(Row, bool)>, max_rows: usize) -> Vec<Row> {
    if max_rows == 0 {
        return Vec::new();
    }
    let budget = max_rows - 1; // the "… N more" row
    let priority = rows.iter().filter(|(_, keep)| *keep).count();
    let mut spare = budget.saturating_sub(priority);
    let mut kept = Vec::new();
    let mut dropped = 0;
    for (row, keep) in rows {
        if kept.len() < budget && (keep || spare > 0) {
            if !keep {
                spare -= 1;
            }
            kept.push(row);
        } else {
            dropped += 1;
        }
    }
    kept.push(Row {
        depth: 1,
        glyph: "…".into(),
        label: format!("{dropped} more"),
        detail: String::new(),
        tone: Tone::More,
    });
    kept
}

/// Labels per step: pending, running, done, failed.
fn labels(
    kind: StepKind,
    unstaged: bool,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match kind {
        StepKind::Backup => (
            "Back up original state",
            "Backing up original state…",
            "Backed up original state",
            "Failed to back up original state",
        ),
        StepKind::Tasks if unstaged => (
            "Run tasks for unstaged files",
            "Running tasks for unstaged files…",
            "Ran tasks for unstaged files",
            "Tasks failed",
        ),
        StepKind::Tasks => (
            "Run tasks for staged files",
            "Running tasks for staged files…",
            "Ran tasks for staged files",
            "Tasks failed",
        ),
        StepKind::Stage => (
            "Stage changes from tasks",
            "Staging changes from tasks…",
            "Staged changes from tasks",
            "Failed to stage changes from tasks",
        ),
        StepKind::Cleanup => (
            "Clean up temporary files",
            "Cleaning up temporary files…",
            "Cleaned up temporary files",
            "Failed to clean up temporary files",
        ),
    }
}

pub fn spinner(since: Duration) -> &'static str {
    SPINNER[(since.as_millis() / SPIN_FRAME.as_millis()) as usize % SPINNER.len()]
}

pub fn pulse(since: Duration) -> usize {
    let step = PULSE_PERIOD.as_millis() / PULSE_STEPS as u128;
    ((since.as_millis() % PULSE_PERIOD.as_millis()) / step) as usize
}

/// The first `ceil(len * t / TYPE_IN)` characters of `label`.
pub fn type_in(label: &str, since: Duration) -> String {
    let len = label.chars().count();
    if since >= TYPE_IN {
        return label.to_string();
    }
    let shown = (len as u128 * since.as_millis()).div_ceil(TYPE_IN.as_millis()) as usize;
    label.chars().take(shown).collect()
}

fn emphasized(finished: Instant, now: Instant) -> bool {
    now.saturating_duration_since(finished) < HIGHLIGHT
}

#[cfg(test)]
mod tests;
