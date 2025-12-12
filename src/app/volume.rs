//! This isn't a real widget. It has no state nor any need for it. Might be expanded into one later.
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{LineGauge, Widget},
};

use crate::audio::Volume;

pub fn render(area: Rect, buf: &mut Buffer, state: &Volume) {
    let label = match state.val() {
        0.8.. => format!("🔊 {}", state),
        0.5.. => format!("🔉 {}", state),
        _ => format!("🔈 {}", state),
    };
    let ratio = state.val() as f64;

    LineGauge::default()
        .ratio(ratio)
        .label(label)
        .style(Style::default().fg(Color::White))
        .filled_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .render(area, buf);
}
