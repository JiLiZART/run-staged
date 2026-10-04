//! Terminal lifecycle: an inline viewport sized from the plan, a render thread
//! fed by a channel, and restoration on finish, drop, and panic.
//!
//! No raw mode: the view only draws, so Ctrl+C stays a normal signal.

use std::{
    io::{self, Stdout, Write},
    sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        cursor::{MoveTo, Show},
        execute,
        style::ResetColor,
    },
    layout::Rect,
    Terminal, TerminalOptions, Viewport,
};

use crate::{
    model::{Event, Model},
    render::{draw, Theme, CHROME_ROWS},
};

/// A message for the render thread: an event and when it happened.
pub type Stamped = (Instant, Event);

/// Minimum time between two frames (at most 20 per second).
const FRAME: Duration = Duration::from_millis(50);
/// Extra rows reserved for output tails of commands running at the same time.
const TAIL_ROWS: usize = 3;

/// Applies events and draws until [`Event::Done`] (or the sender is dropped),
/// then draws the final frame. The terminal is created on the plan event by
/// `make_terminal(desired_height)`. Returns the terminal and the final frame's
/// area so the caller can restore below it.
pub fn render_loop<B: Backend>(
    rx: Receiver<Stamped>,
    mut make_terminal: impl FnMut(u16) -> Result<Terminal<B>, B::Error>,
    clock: impl Fn() -> Instant,
    theme: Theme,
) -> Result<Option<(Terminal<B>, Rect)>, B::Error> {
    let mut model = Model::default();
    let mut terminal: Option<Terminal<B>> = None;
    let mut last_draw: Option<Instant> = None;
    let mut area = Rect::default();
    loop {
        let mut done = false;
        let mut apply = |(at, ev): Stamped, model: &mut Model| -> bool {
            let is_plan = matches!(ev, Event::Plan(_));
            done |= matches!(ev, Event::Done);
            model.apply(ev, at);
            is_plan
        };
        let mut got_event = false;
        match rx.recv_timeout(FRAME) {
            Ok(msg) => {
                got_event = true;
                let mut planned = apply(msg, &mut model);
                // Coalesce everything already queued (e.g. bursts of output lines).
                while let Ok(msg) = rx.try_recv() {
                    planned |= apply(msg, &mut model);
                }
                if planned && terminal.is_none() {
                    let rows = model.rows(clock(), usize::MAX).len();
                    let desired = rows + TAIL_ROWS + CHROME_ROWS as usize;
                    terminal = Some(make_terminal(desired.min(u16::MAX as usize) as u16)?);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => done = true,
        }
        if let Some(t) = terminal.as_mut() {
            let now = clock();
            let due = last_draw.is_none_or(|l| now.saturating_duration_since(l) >= FRAME);
            if done || (due && (got_event || !model.is_done())) {
                if done && !model.is_done() {
                    model.apply(Event::Done, now);
                }
                // `f.area()` is the inline viewport; `CompletedFrame::area` is not.
                t.draw(|f| {
                    area = f.area();
                    draw(f, area, &model, now, theme);
                })?;
                last_draw = Some(now);
            }
        }
        if done {
            return Ok(terminal.map(|t| (t, area)));
        }
    }
}

/// Writes the restore sequence: visible cursor, default colors, fresh line.
pub fn restore_sequence(w: &mut impl Write) -> io::Result<()> {
    execute!(w, ResetColor, Show)?;
    w.write_all(b"\r\n")?;
    w.flush()
}

/// The live view on stdout. Send events through [`LiveView::sender`] and call
/// [`LiveView::finish`] when the run is over.
pub struct LiveView {
    tx: Sender<Stamped>,
    handle: Option<JoinHandle<io::Result<()>>>,
}

impl LiveView {
    /// Starts the render thread and installs a panic hook that restores the
    /// terminal (needed because release builds abort on panic, skipping `Drop`).
    pub fn start(theme: Theme) -> LiveView {
        install_panic_hook();
        let (tx, rx) = channel::<Stamped>();
        let handle = std::thread::spawn(move || {
            let drawn = render_loop(rx, inline_terminal, Instant::now, theme)?;
            if let Some((mut terminal, area)) = drawn {
                // Put the cursor on the frame's last row; the restore newline
                // then moves below it, keeping the final frame in scrollback.
                let backend = terminal.backend_mut();
                execute!(backend, MoveTo(0, area.bottom().saturating_sub(1)))?;
                restore_sequence(backend)?;
            }
            Ok(())
        });
        LiveView {
            tx,
            handle: Some(handle),
        }
    }

    /// A handle for sending events from any thread.
    pub fn sender(&self) -> Sender<Stamped> {
        self.tx.clone()
    }

    /// Draws the final frame, restores the terminal, and waits for the thread.
    pub fn finish(mut self) -> io::Result<()> {
        self.join()
    }

    fn join(&mut self) -> io::Result<()> {
        let _ = self.tx.send((Instant::now(), Event::Done));
        match self.handle.take() {
            Some(h) => h.join().unwrap_or(Ok(())),
            None => Ok(()),
        }
    }
}

impl Drop for LiveView {
    fn drop(&mut self) {
        let _ = self.join();
    }
}

/// Terminal width in columns, if the terminal can tell.
pub fn terminal_width() -> Option<usize> {
    ratatui::crossterm::terminal::size()
        .ok()
        .map(|(cols, _)| cols as usize)
}

/// An inline viewport at most one row shorter than the terminal.
fn inline_terminal(desired: u16) -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    let (_, rows) = ratatui::crossterm::terminal::size()?;
    let height = desired.min(rows.saturating_sub(1)).max(CHROME_ROWS + 1);
    let mut terminal = Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        TerminalOptions {
            viewport: Viewport::Inline(height),
        },
    )?;
    terminal.hide_cursor()?;
    Ok(terminal)
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_sequence(&mut io::stdout());
        previous(info);
    }));
}

#[cfg(test)]
mod tests;
