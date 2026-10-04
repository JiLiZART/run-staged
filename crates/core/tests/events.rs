//! `StepEvent::Planned` and exec events reach the caller in run order.

use std::sync::{Arc, Mutex};

use run_staged_core::{Plan, RunOptions, Step, StepEvent};
use run_staged_exec::{ExecEvent, ExecOptions};
use run_staged_testkit::{fixture_tool_path, Repo};

#[test]
fn planned_first_then_backup_then_command_events_inside_tasks() {
    let tool = shlex::try_quote(&fixture_tool_path().to_string_lossy())
        .unwrap()
        .into_owned();
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .staged("b.rs", "b")
        .file(
            "run-staged.toml",
            format!(
                "[fmt]\npatterns = {{ \"*.js\" = ['{tool} --stdout hi'] }}\n\
                 [lint]\npatterns = {{ \"*.rs\" = ['{tool}', '{tool}'] }}\n"
            ),
        )
        .build();

    // One timeline for both callbacks, like the binary's channel.
    let timeline: Arc<Mutex<Vec<String>>> = Arc::default();
    let exec_log = Arc::clone(&timeline);
    let opts = RunOptions {
        cwd: repo.root().to_path_buf(),
        exec: ExecOptions {
            events: Some(Arc::new(move |e| {
                let s = match e {
                    ExecEvent::Started { id } => format!("cmd-start {id}"),
                    ExecEvent::Line { id, text } => format!("cmd-line {id} {text}"),
                    ExecEvent::Finished { id, .. } => format!("cmd-done {id}"),
                };
                exec_log.lock().unwrap().push(s);
            })),
            force_color: false,
        },
        ..Default::default()
    };
    let mut plan: Option<Plan> = None;
    let step_log = Arc::clone(&timeline);
    let mut on_step = |e: StepEvent<'_>| {
        let s = match e {
            StepEvent::Planned(p) => {
                plan = Some(p.clone());
                "planned".to_string()
            }
            StepEvent::Started(s) => format!("start {s:?}"),
            StepEvent::Finished(s) => format!("done {s:?}"),
            StepEvent::Failed(s) => format!("fail {s:?}"),
            StepEvent::Outcomes(_) => "outcomes".to_string(),
        };
        step_log.lock().unwrap().push(s);
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let report = rt
        .block_on(run_staged_core::run(opts, &mut on_step))
        .unwrap();
    assert!(report.error.is_none(), "{:?}", report.error);

    let t = timeline.lock().unwrap().clone();
    assert_eq!(t[0], "planned");
    assert_eq!(t[1], format!("start {:?}", Step::Backup));
    let tasks_start = t.iter().position(|s| s == "start Tasks").unwrap();
    let tasks_end = t.iter().position(|s| s == "done Tasks").unwrap();
    for (i, s) in t.iter().enumerate() {
        if s.starts_with("cmd-") {
            assert!(
                tasks_start < i && i < tasks_end,
                "{s} outside tasks step: {t:?}"
            );
        }
    }
    assert!(t.contains(&"cmd-line 0 hi".to_string()), "{t:?}");

    let plan = plan.unwrap();
    assert_eq!(plan.file_count, 2);
    assert_eq!(
        plan.groups
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>(),
        ["fmt", "lint"]
    );
    assert_eq!(plan.groups[1].file_count, 1);
    assert_eq!(plan.invocations.len(), 3);
    for inv in &plan.invocations {
        assert!(
            t.contains(&format!("cmd-done {}", inv.id)),
            "id {} never finished",
            inv.id
        );
    }
}
