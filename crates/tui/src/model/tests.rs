use std::time::{Duration, Instant};

use run_staged_exec::{CommandOutcome, ExecEvent, PlannedInvocation};

use super::*;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn inv(id: usize, group: &str, command: &str) -> PlannedInvocation {
    PlannedInvocation {
        id,
        group: group.into(),
        pattern: "*".into(),
        command: command.into(),
        file_count: 1,
    }
}

fn plan(unstaged: bool, invs: Vec<PlannedInvocation>, groups: &[(&str, &str, usize)]) -> PlanInfo {
    PlanInfo {
        unstaged,
        file_count: 3,
        groups: groups
            .iter()
            .map(|(n, p, f)| PlanGroup {
                name: (*n).into(),
                patterns: vec![(*p).into()],
                file_count: *f,
            })
            .collect(),
        invocations: invs,
    }
}

fn success() -> CommandOutcome {
    CommandOutcome::Success {
        group: "g".into(),
        pattern: "*".into(),
        command: "c".into(),
        argv: vec![],
        file_count: 1,
        duration: Duration::ZERO,
        stdout: vec![],
    }
}

fn failure(timed_out: bool) -> CommandOutcome {
    CommandOutcome::Failure {
        group: "g".into(),
        pattern: "*".into(),
        command: "c".into(),
        argv: vec![],
        file_count: 1,
        duration: Duration::ZERO,
        exit_code: 1,
        stdout: vec![],
        stderr: vec![],
        timed_out,
    }
}

fn cancelled() -> CommandOutcome {
    CommandOutcome::Cancelled {
        group: "g".into(),
        pattern: "*".into(),
        command: "c".into(),
    }
}

/// A model with one group `format` (`prettier`, `eslint`) and t0.
fn base() -> (Model, Instant) {
    let t0 = Instant::now();
    let mut m = Model::default();
    m.apply(
        Event::Plan(plan(
            false,
            vec![
                inv(0, "format", "prettier --write"),
                inv(1, "format", "eslint --fix"),
            ],
            &[("format", "*.{js,ts}", 2)],
        )),
        t0,
    );
    (m, t0)
}

fn labels_of(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|r| format!("{}{} {}", "  ".repeat(r.depth as usize), r.glyph, r.label))
        .collect()
}

#[test]
fn steps_before_tasks_start() {
    let (mut m, t0) = base();
    m.apply(Event::StepStarted(StepKind::Backup), t0);
    let rows = m.rows(t0 + ms(500), 50);
    assert!(matches!(rows[0].tone, Tone::Running { .. }));
    assert!(SPINNER.contains(&rows[0].glyph.as_str()));
    let pending: Vec<_> = rows
        .iter()
        .filter(|r| r.depth == 0)
        .skip(1)
        .map(|r| (r.glyph.as_str(), r.tone))
        .collect();
    assert_eq!(pending, vec![("◌", Tone::Pending); 3]);
}

#[test]
fn group_with_two_commands() {
    let (m, t0) = base();
    let rows = m.rows(t0, 50);
    let g = rows.iter().find(|r| r.depth == 1).unwrap();
    assert_eq!(
        (g.glyph.as_str(), g.label.as_str(), g.detail.as_str()),
        ("▸", "format  *.{js,ts}", "2 files")
    );
    let cmds: Vec<_> = rows
        .iter()
        .filter(|r| r.depth == 2)
        .map(|r| r.label.as_str())
        .collect();
    assert_eq!(cmds, vec!["prettier --write", "eslint --fix"]);
}

#[test]
fn six_command_states() {
    let t0 = Instant::now();
    let mut m = Model::default();
    let invs = (0..6).map(|i| inv(i, "g", &format!("c{i}"))).collect();
    m.apply(Event::Plan(plan(false, invs, &[("g", "*", 6)])), t0);
    for id in 1..6 {
        m.apply(Event::Exec(ExecEvent::Started { id }), t0);
    }
    let fin = |m: &mut Model, id, outcome| {
        m.apply(
            Event::Exec(ExecEvent::Finished { id, outcome }),
            t0 + ms(10),
        )
    };
    fin(&mut m, 2, success());
    fin(&mut m, 3, failure(false));
    fin(&mut m, 4, failure(true));
    fin(&mut m, 5, cancelled());
    let rows = m.rows(t0 + ms(1000), 50);
    let glyphs: Vec<_> = rows
        .iter()
        .filter(|r| r.depth == 2)
        .map(|r| r.glyph.as_str())
        .collect();
    assert_eq!(glyphs[0], "◌");
    assert!(SPINNER.contains(&glyphs[1]));
    assert_eq!(&glyphs[2..], ["✔", "✗", "⏱", "⊘"]);
}

#[test]
fn failed_run_final_frame_omits_steps_that_never_ran() {
    let (mut m, t0) = base();
    for (e, t) in [
        (Event::StepStarted(StepKind::Backup), 0),
        (Event::StepFinished(StepKind::Backup), 5),
        (Event::StepStarted(StepKind::Tasks), 5),
        (Event::StepFailed(StepKind::Tasks), 50),
        (Event::StepStarted(StepKind::Cleanup), 50),
        (Event::StepFinished(StepKind::Cleanup), 60),
        (Event::Done, 60),
    ] {
        m.apply(e, t0 + ms(t));
    }
    let steps: Vec<_> = m
        .rows(t0 + ms(2000), 50)
        .into_iter()
        .filter(|r| r.depth == 0)
        .map(|r| (r.glyph, r.label))
        .collect();
    assert_eq!(
        steps,
        vec![
            ("✔".to_string(), "Backed up original state".to_string()),
            ("✗".to_string(), "Tasks failed".to_string()),
            ("✔".to_string(), "Cleaned up temporary files".to_string()),
        ]
    );
}

#[test]
fn unstaged_lists_only_tasks_step() {
    let t0 = Instant::now();
    let mut m = Model::default();
    m.apply(
        Event::Plan(plan(true, vec![inv(0, "g", "c")], &[("g", "*", 1)])),
        t0,
    );
    let steps: Vec<_> = m
        .rows(t0, 50)
        .into_iter()
        .filter(|r| r.depth == 0)
        .map(|r| r.label)
        .collect();
    assert_eq!(steps, vec!["Run tasks for unstaged files"]);
}

#[test]
fn group_order_follows_plan() {
    let t0 = Instant::now();
    let mut m = Model::default();
    m.apply(
        Event::Plan(plan(
            false,
            vec![inv(0, "zeta", "z"), inv(1, "alpha", "a")],
            &[("zeta", "*", 1), ("alpha", "*", 1)],
        )),
        t0,
    );
    let groups: Vec<_> = m
        .rows(t0, 50)
        .into_iter()
        .filter(|r| r.depth == 1)
        .map(|r| r.label)
        .collect();
    assert_eq!(groups, vec!["zeta  *", "alpha  *"]);
}

#[test]
fn tail_shows_newest_sanitized_line_and_is_removed_on_finish() {
    let (mut m, t0) = base();
    m.apply(Event::Exec(ExecEvent::Started { id: 0 }), t0);
    for text in ["checking src/a.ts", "\x1b[31merror\x1b[0m: bad", "   "] {
        m.apply(
            Event::Exec(ExecEvent::Line {
                id: 0,
                text: text.into(),
            }),
            t0,
        );
    }
    let tail = |m: &Model| {
        m.rows(t0, 50)
            .into_iter()
            .find(|r| r.tone == Tone::Tail)
            .map(|r| r.label)
    };
    assert_eq!(
        tail(&m).as_deref(),
        Some("error: bad"),
        "blank lines are ignored"
    );
    m.apply(
        Event::Exec(ExecEvent::Finished {
            id: 0,
            outcome: success(),
        }),
        t0,
    );
    assert_eq!(tail(&m), None);
}

#[test]
fn spinner_frames() {
    assert_eq!(spinner(ms(0)), SPINNER[0]);
    assert_eq!(spinner(ms(79)), SPINNER[0]);
    assert_eq!(spinner(ms(80)), SPINNER[1]);
    assert_eq!(spinner(ms(160)), SPINNER[2]);
    assert_eq!(spinner(ms(800)), SPINNER[0]);
}

#[test]
fn pulse_steps() {
    assert_eq!(pulse(ms(0)), 0);
    assert_eq!(pulse(ms(199)), 0);
    assert_eq!(pulse(ms(200)), 1);
    assert_eq!(pulse(ms(1100)), 5);
    assert_eq!(pulse(ms(1200)), 0);
}

#[test]
fn type_in_reveals_label() {
    let label = "Backing up…"; // 11 chars
    assert_eq!(type_in(label, ms(0)), "");
    assert_eq!(type_in(label, ms(75)).chars().count(), 6);
    assert_eq!(type_in(label, ms(150)), label);
    assert_eq!(type_in(label, ms(9999)), label);
}

#[test]
fn completion_highlight_lasts_300ms() {
    let (mut m, t0) = base();
    m.apply(Event::StepStarted(StepKind::Backup), t0);
    m.apply(Event::StepFinished(StepKind::Backup), t0 + ms(4));
    let backup = |now| m.rows(t0 + ms(now), 50).remove(0);
    let r = backup(4 + 299);
    assert_eq!((r.glyph.as_str(), r.tone), ("✔", Tone::Done { emph: true }));
    assert_eq!(r.detail, "4ms");
    assert_eq!(backup(4 + 300).tone, Tone::Done { emph: false });
}

#[test]
fn running_rows_carry_elapsed_time() {
    let (mut m, t0) = base();
    m.apply(Event::Exec(ExecEvent::Started { id: 0 }), t0);
    let detail = |now| {
        m.rows(t0 + ms(now), 50)
            .into_iter()
            .find(|r| r.label == "prettier --write")
            .unwrap()
            .detail
    };
    assert_eq!(detail(500), "500ms");
    assert_eq!(detail(3000), "3.00s");
}

#[test]
fn progress_and_overall() {
    let (mut m, t0) = base();
    assert_eq!(m.progress(), (0, 2));
    assert_eq!(m.overall(), Overall::Running);
    m.apply(
        Event::Exec(ExecEvent::Finished {
            id: 0,
            outcome: success(),
        }),
        t0,
    );
    assert_eq!(m.progress(), (1, 2));
    m.apply(
        Event::Exec(ExecEvent::Finished {
            id: 1,
            outcome: failure(false),
        }),
        t0,
    );
    assert_eq!(m.overall(), Overall::Failed);
    let (mut ok, t0) = base();
    for id in 0..2 {
        ok.apply(
            Event::Exec(ExecEvent::Finished {
                id,
                outcome: success(),
            }),
            t0,
        );
    }
    ok.apply(Event::Done, t0);
    assert_eq!(ok.overall(), Overall::Succeeded);
}

#[test]
fn fit_drops_tails_then_collapses_then_truncates() {
    let t0 = Instant::now();
    let mut m = Model::default();
    // 3 groups x 10 commands = 30 commands.
    let invs: Vec<_> = (0..30)
        .map(|i| inv(i, &format!("g{}", i / 10), &format!("cmd{i}")))
        .collect();
    m.apply(
        Event::Plan(plan(
            false,
            invs,
            &[("g0", "*", 10), ("g1", "*", 10), ("g2", "*", 10)],
        )),
        t0,
    );
    m.apply(Event::StepStarted(StepKind::Tasks), t0);
    // g0 finished, g1 has two running commands with tails, g2 queued.
    for id in 0..10 {
        m.apply(
            Event::Exec(ExecEvent::Finished {
                id,
                outcome: success(),
            }),
            t0,
        );
    }
    for id in [14, 17] {
        m.apply(Event::Exec(ExecEvent::Started { id }), t0);
        m.apply(
            Event::Exec(ExecEvent::Line {
                id,
                text: "progress".into(),
            }),
            t0,
        );
    }

    let full = m.rows(t0, 1000);
    assert!(full.iter().any(|r| r.tone == Tone::Tail));

    // A 14-row terminal: 13 rows for the panel, minus 2 borders and 1 gauge = 10.
    let rows = m.rows(t0 + ms(1000), 10);
    assert!(rows.len() <= 10, "{:#?}", labels_of(&rows));
    assert!(!rows.iter().any(|r| r.tone == Tone::Tail), "tails go first");
    let text = labels_of(&rows);
    for running in ["cmd14", "cmd17"] {
        assert!(
            text.iter().any(|l| l.ends_with(running)),
            "{running} hidden: {text:#?}"
        );
    }
    assert!(
        !text.iter().any(|l| l.ends_with("cmd3")),
        "finished group collapsed: {text:#?}"
    );
    assert_eq!(rows.last().unwrap().tone, Tone::More);
    assert!(rows.last().unwrap().label.ends_with("more"));

    // Without tails and with collapse it would fit a larger budget exactly.
    let roomy = m.rows(t0, 30);
    assert!(roomy.len() <= 30);
    assert!(roomy.iter().all(|r| r.tone != Tone::More));
}

#[test]
fn elapsed_freezes_when_done() {
    let (mut m, t0) = base();
    m.apply(Event::Done, t0 + ms(1240));
    assert_eq!(m.elapsed(t0 + ms(9999)), ms(1240));
}
