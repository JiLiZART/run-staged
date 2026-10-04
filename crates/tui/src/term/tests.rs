use std::{
    sync::mpsc::channel,
    time::{Duration, Instant},
};

use ratatui::{backend::TestBackend, Terminal};
use run_staged_exec::{CommandOutcome, ExecEvent, PlannedInvocation};

use super::{render_loop, restore_sequence, Stamped};
use crate::{
    model::{Event, PlanGroup, PlanInfo, StepKind},
    render::Theme,
};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn restore_sequence_shows_cursor_and_resets_colors() {
    let mut out = Vec::new();
    restore_sequence(&mut out).unwrap();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("\x1b[?25h"), "show cursor: {s:?}");
    assert!(s.contains("\x1b[0m"), "reset colors: {s:?}");
    assert!(s.ends_with("\r\n"), "fresh line: {s:?}");
}

#[test]
fn scripted_run_ends_on_final_frame() {
    let t0 = Instant::now();
    let (tx, rx) = channel::<Stamped>();
    let ok = CommandOutcome::Success {
        group: String::new(),
        pattern: String::new(),
        command: String::new(),
        argv: vec![],
        file_count: 1,
        duration: Duration::ZERO,
        stdout: vec![],
    };
    let script = vec![
        (
            0,
            Event::Plan(PlanInfo {
                unstaged: false,
                file_count: 1,
                groups: vec![PlanGroup {
                    name: "fmt".into(),
                    patterns: vec!["*.js".into()],
                    file_count: 1,
                }],
                invocations: vec![PlannedInvocation {
                    id: 0,
                    group: "fmt".into(),
                    pattern: "*.js".into(),
                    command: "prettier --write".into(),
                    file_count: 1,
                }],
            }),
        ),
        (0, Event::StepStarted(StepKind::Backup)),
        (3, Event::StepFinished(StepKind::Backup)),
        (3, Event::StepStarted(StepKind::Tasks)),
        (4, Event::Exec(ExecEvent::Started { id: 0 })),
        (
            50,
            Event::Exec(ExecEvent::Line {
                id: 0,
                text: "formatting".into(),
            }),
        ),
        (120, Event::Exec(ExecEvent::Finished { id: 0, outcome: ok })),
        (120, Event::StepFinished(StepKind::Tasks)),
        (120, Event::StepStarted(StepKind::Stage)),
        (125, Event::StepFinished(StepKind::Stage)),
        (125, Event::StepStarted(StepKind::Cleanup)),
        (126, Event::StepFinished(StepKind::Cleanup)),
        (126, Event::Done),
    ];
    for (at, e) in script {
        tx.send((t0 + ms(at), e)).unwrap();
    }
    let mut asked_height = 0;
    let (terminal, area) = render_loop(
        rx,
        |h| {
            asked_height = h;
            Terminal::new(TestBackend::new(50, h))
        },
        || t0 + ms(2000),
        Theme { color: true },
    )
    .unwrap()
    .expect("terminal created on plan");
    assert_eq!(area.height, asked_height);
    let buf = terminal.backend().buffer();
    let text: String = (0..buf.area.height)
        .map(|y| {
            let line: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            line.trim_end().to_string() + "\n"
        })
        .collect();
    insta::assert_snapshot!(text);
    assert!(!text.contains("formatting"), "no tails in the final frame");
    assert!(text.contains("1/1"));
}

#[test]
fn dropped_sender_still_draws_final_frame() {
    let t0 = Instant::now();
    let (tx, rx) = channel::<Stamped>();
    tx.send((
        t0,
        Event::Plan(PlanInfo {
            unstaged: true,
            file_count: 0,
            groups: vec![],
            invocations: vec![],
        }),
    ))
    .unwrap();
    drop(tx);
    let drawn = render_loop(
        rx,
        |h| Terminal::new(TestBackend::new(40, h)),
        || t0,
        Theme { color: false },
    )
    .unwrap();
    assert!(drawn.is_some());
}
