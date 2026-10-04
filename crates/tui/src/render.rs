//! Draws the panel: a rounded, state-colored `Block` with titles, a `List` of
//! the model's rows, and a `LineGauge` of finished/total invocations.

use std::time::Instant;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, BorderType, LineGauge, List, ListItem, Padding},
    Frame,
};
use run_staged_output::format_duration;

use crate::model::{Model, Overall, Row, Tone};

/// Cyan brightness ramp for running rows (dim → bright → dim).
const PULSE: [Color; 6] = [
    Color::Indexed(30),
    Color::Indexed(37),
    Color::Indexed(44),
    Color::Indexed(51),
    Color::Indexed(44),
    Color::Indexed(37),
];

/// Whether the panel may use colors (`NO_COLOR` unset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub color: bool,
}

impl Theme {
    fn fg(self, c: Color) -> Style {
        if self.color {
            Style::new().fg(c)
        } else {
            Style::new()
        }
    }

    fn dim(self) -> Style {
        self.fg(Color::DarkGray).add_modifier(Modifier::DIM)
    }

    fn state(self, o: Overall) -> Style {
        self.fg(match o {
            Overall::Running => Color::Cyan,
            Overall::Succeeded => Color::Green,
            Overall::Failed => Color::Red,
        })
    }

    /// Style of a row's glyph and label.
    fn tone(self, t: Tone) -> Style {
        let emph = |base: Color, bright: Color, on: bool| {
            if on {
                self.fg(bright).add_modifier(Modifier::BOLD)
            } else {
                self.fg(base)
            }
        };
        match t {
            Tone::Pending | Tone::Cancelled | Tone::Tail | Tone::More => self.dim(),
            Tone::Running { pulse } => self
                .fg(PULSE[pulse % PULSE.len()])
                .add_modifier(Modifier::BOLD),
            Tone::Done { emph: e } => emph(Color::Green, Color::LightGreen, e),
            Tone::Failed { emph: e } => emph(Color::Red, Color::LightRed, e),
            Tone::TimedOut { emph: e } => emph(Color::Yellow, Color::LightYellow, e),
            Tone::Group => Style::new().add_modifier(Modifier::BOLD),
        }
    }
}

/// Rows the panel needs for `content_rows` lines of tree (borders + gauge).
pub const CHROME_ROWS: u16 = 3;

/// Draws the whole panel into `area`.
pub fn draw(frame: &mut Frame, area: Rect, model: &Model, now: Instant, theme: Theme) {
    let overall = model.overall();
    let files = if model.file_count() == 1 {
        "file"
    } else {
        "files"
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .border_style(theme.state(overall))
        .title_top(
            Line::from(" run-staged ").style(theme.state(overall).add_modifier(Modifier::BOLD)),
        )
        .title_top(
            Line::from(format!(
                " {} {files} · {} ",
                model.file_count(),
                format_duration(model.elapsed(now))
            ))
            .right_aligned(),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [list_area, gauge_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);
    let rows = model.rows(now, list_area.height as usize);
    let items: Vec<ListItem> = rows
        .iter()
        .map(|r| ListItem::new(row_line(r, theme, list_area.width)))
        .collect();
    frame.render_widget(List::new(items), list_area);

    let (done, total) = model.progress();
    let ratio = if total == 0 {
        1.0
    } else {
        done as f64 / total as f64
    };
    frame.render_widget(
        LineGauge::default()
            .ratio(ratio)
            .label(format!("{done}/{total}"))
            .filled_symbol(symbols::line::THICK_HORIZONTAL)
            .unfilled_symbol(symbols::line::HORIZONTAL)
            .filled_style(theme.state(overall))
            .unfilled_style(theme.dim()),
        gauge_area,
    );
}

/// `<indent><glyph> <label>  <detail>`, the label truncated with `…` to fit.
fn row_line(r: &Row, theme: Theme, width: u16) -> Line<'static> {
    let indent = "  ".repeat(r.depth as usize);
    let style = theme.tone(r.tone);
    let detail = if r.detail.is_empty() {
        String::new()
    } else {
        format!("  {}", r.detail)
    };
    let fixed = indent.chars().count() + r.glyph.chars().count() + 1 + detail.chars().count();
    let room = (width as usize).saturating_sub(fixed);
    let label = truncate(&r.label, room);
    // Settled success rows keep a calm label; the green lives in the glyph.
    let label_style = match r.tone {
        Tone::Done { emph: false } => Style::new(),
        _ => style,
    };
    Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{} ", r.glyph), style),
        Span::styled(label, label_style),
        Span::styled(detail, theme.dim()),
    ])
}

fn truncate(s: &str, room: usize) -> String {
    if s.chars().count() <= room {
        return s.to_string();
    }
    if room == 0 {
        return String::new();
    }
    let mut t: String = s.chars().take(room - 1).collect();
    t.push('…');
    t
}

#[cfg(test)]
mod tests;
