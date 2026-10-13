use std::time::{Duration, Instant};

use ratatui::{Terminal, backend::TestBackend, style::Color};
use run_staged_exec::{CommandOutcome, ExecEvent, PlannedInvocation};

use super::{Theme, draw};
use crate::model::{Event, Model, PlanGroup, PlanInfo, StepKind};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

const COLOR: Theme = Theme { color: true };

fn render(model: &Model, now: Instant, w: u16, h: u16, theme: Theme) -> Terminal<TestBackend> {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| draw(f, f.area(), model, now, theme)).unwrap();
    t
}

fn text(t: &Terminal<TestBackend>) -> String {
    let buf = t.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
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

fn plan() -> PlanInfo {
    PlanInfo {
        unstaged: false,
        file_count: 3,
        groups: vec![
            PlanGroup {
                name: "format".into(),
                patterns: vec!["*.{js,ts}".into()],
                file_count: 2,
            },
            PlanGroup {
                name: "rust".into(),
                patterns: vec!["*.rs".into()],
                file_count: 1,
            },
        ],
        invocations: vec![
            inv(0, "format", "prettier --write"),
            inv(1, "format", "eslint --fix"),
            inv(2, "rust", "cargo fmt"),
            inv(3, "rust", "cargo clippy"),
            inv(4, "rust", "cargo test"),
            inv(5, "rust", "taplo fmt"),
            inv(6, "rust", "typos"),
        ],
    }
}

fn ok() -> CommandOutcome {
    CommandOutcome::Success {
        group: String::new(),
        pattern: String::new(),
        command: String::new(),
        argv: vec![],
        file_count: 1,
        duration: Duration::ZERO,
        stdout: vec![],
    }
}

fn failed() -> CommandOutcome {
    CommandOutcome::Failure {
        group: String::new(),
        pattern: String::new(),
        command: String::new(),
        argv: vec![],
        file_count: 1,
        duration: Duration::ZERO,
        exit_code: 1,
        stdout: vec![],
        stderr: vec![],
        timed_out: false,
    }
}

fn model_with(events: &[(u64, Event)]) -> (Model, Instant) {
    let t0 = Instant::now();
    let mut m = Model::default();
    m.apply(Event::Plan(plan()), t0);
    for (at, e) in events {
        m.apply(e.clone(), t0 + ms(*at));
    }
    (m, t0)
}

#[test]
fn backup_running() {
    let (m, t0) = model_with(&[(0, Event::StepStarted(StepKind::Backup))]);
    insta::assert_snapshot!(text(&render(&m, t0 + ms(400), 60, 16, COLOR)));
}

#[test]
fn mixed_tasks_tree_with_tails_and_gauge() {
    let mut ev = vec![
        (0, Event::StepStarted(StepKind::Backup)),
        (4, Event::StepFinished(StepKind::Backup)),
        (4, Event::StepStarted(StepKind::Tasks)),
    ];
    for id in 0..7 {
        ev.push((5, Event::Exec(ExecEvent::Started { id })));
    }
    for (id, at) in [(1, 645), (2, 215), (3, 900)] {
        ev.push((at, Event::Exec(ExecEvent::Finished { id, outcome: ok() })));
    }
    ev.push((
        700,
        Event::Exec(ExecEvent::Line {
            id: 0,
            text: "checking src/b.ts".into(),
        }),
    ));
    let (m, t0) = model_with(&ev);
    insta::assert_snapshot!(text(&render(&m, t0 + ms(1205), 60, 20, COLOR)));
    assert!(text(&render(&m, t0 + ms(1205), 60, 20, COLOR)).contains("3/7"));
}

#[test]
fn failed_final_frame_has_red_border() {
    let mut ev = vec![
        (0, Event::StepStarted(StepKind::Backup)),
        (4, Event::StepFinished(StepKind::Backup)),
        (4, Event::StepStarted(StepKind::Tasks)),
    ];
    for id in 0..7 {
        ev.push((5, Event::Exec(ExecEvent::Started { id })));
        let outcome = if id == 3 { failed() } else { ok() };
        ev.push((300, Event::Exec(ExecEvent::Finished { id, outcome })));
    }
    ev.extend([
        (300, Event::StepFailed(StepKind::Tasks)),
        (300, Event::StepStarted(StepKind::Cleanup)),
        (310, Event::StepFinished(StepKind::Cleanup)),
        (310, Event::Done),
    ]);
    let (m, t0) = model_with(&ev);
    let t = render(&m, t0 + ms(2000), 60, 16, COLOR);
    insta::assert_snapshot!(text(&t));
    assert_eq!(t.backend().buffer()[(0, 0)].fg, Color::Red, "border is red");
}

#[test]
fn small_terminal() {
    let mut ev = vec![(0, Event::StepStarted(StepKind::Tasks))];
    for id in [0, 4] {
        ev.push((0, Event::Exec(ExecEvent::Started { id })));
    }
    let (m, t0) = model_with(&ev);
    let t = render(&m, t0 + ms(100), 50, 9, COLOR);
    let s = text(&t);
    insta::assert_snapshot!(s);
    assert!(s.contains("more"), "{s}");
    assert!(
        s.contains("prettier") && s.contains("cargo test"),
        "running rows kept: {s}"
    );
}

#[test]
fn no_color_has_no_foreground_colors() {
    let (m, t0) = model_with(&[
        (0, Event::StepStarted(StepKind::Backup)),
        (4, Event::StepFinished(StepKind::Backup)),
        (4, Event::StepStarted(StepKind::Tasks)),
        (5, Event::Exec(ExecEvent::Started { id: 0 })),
    ]);
    let t = render(&m, t0 + ms(100), 60, 16, Theme { color: false });
    let buf = t.backend().buffer();
    for cell in buf.content() {
        assert_eq!(
            cell.fg,
            Color::Reset,
            "cell {:?} has a color",
            cell.symbol()
        );
    }
    let colored = render(&m, t0 + ms(100), 60, 16, COLOR);
    assert!(
        colored
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|c| c.fg != Color::Reset)
    );
}

#[test]
fn long_labels_are_truncated_with_ellipsis() {
    let t0 = Instant::now();
    let mut m = Model::default();
    let mut p = plan();
    p.invocations[0].command = "x".repeat(200);
    m.apply(Event::Plan(p), t0);
    let s = text(&render(&m, t0, 40, 16, COLOR));
    assert!(s.lines().all(|l| l.chars().count() <= 40));
    assert!(s.contains('…'));
}
