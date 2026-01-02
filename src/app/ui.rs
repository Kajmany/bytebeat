use crate::{
    App,
    app::{View, input::BeatInput, volume},
    audio::StreamStatus,
};

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

/// Used in calculation for [`crate::app::BeatInput::height_hint`]
/// May show another line for more errors
/// Not everything wrong becomes a discrete error, it's actually hard to rack up this many
pub const MAX_ERRORS_SHOWN: usize = 3;

const HELP_TEXT: &[&str] = &[
    "Controls:",
    "  F1: Help",
    "  F2: Log",
    "  F3: Quit",
    "  F4: Play/Pause",
    "  F5: Library",
    "  F6: About",
    "  Up/Down: Volume",
    "  Esc: Back to Main",
    "",
    "Input Controls:",
    "  Type to insert characters",
    "  Backspace: Remove character before cursor",
    "  Left/Right: Move cursor",
    "  Ctrl+Left/Right: Jump words",
    "  Enter: Compile and play beat",
];

impl<I: BeatInput> Widget for &mut App<I> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let main_block = Block::bordered()
            .title(" bytebeat   ")
            .title_alignment(Alignment::Left)
            .border_type(BorderType::Rounded)
            .title_bottom(controls(self));

        let main_interior = Layout::default()
            .direction(Direction::Vertical)
            // One big widget area, and a little bottom bar
            .constraints(vec![
                // Scope
                Constraint::Percentage(80),
                // Logs
                Constraint::Percentage(15),
                // Input
                Constraint::Length(self.beat_input.height_hint()),
                // Status bar
                Constraint::Length(3),
            ])
            .split(main_block.inner(area));

        let status_block = Block::new()
            .borders(Borders::TOP)
            .border_type(BorderType::Plain);

        let stream_status = match self.audio_state {
            StreamStatus::Error => "Audio: Error!",
            StreamStatus::Unconnected => "Audio: Unconnected",
            StreamStatus::Connecting => "Audio: Connecting",
            StreamStatus::Paused => "Audio: Paused",
            StreamStatus::Streaming => "Audio: Streaming",
        };

        main_block.render(area, buf);

        // Waveform visualization
        self.scope.render(main_interior[0], buf);

        tui_logger::TuiLoggerWidget::default()
            .block(Block::bordered().title(" Log "))
            .output_separator('|')
            .output_timestamp(Some("%H:%M:%S".to_string()))
            .output_level(Some(tui_logger::TuiLoggerLevelOutput::Long))
            .output_target(false)
            .output_file(false)
            .output_line(false)
            .render(main_interior[1], buf);

        self.beat_input.render(main_interior[2], buf);
        // Status bar text must be rendered before status bar
        let status_area = status_block.inner(main_interior[3]);
        let status_layout = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(30),
            Constraint::Length(1),
        ])
        .split(status_area);

        Paragraph::new(stream_status)
            .centered()
            .style(Style::default().add_modifier(Modifier::BOLD))
            .render(status_layout[0], buf);

        volume::render(status_layout[1], buf, &self.audio_vol);
        status_block.render(main_interior[3], buf);

        if self.view != View::Main {
            let (title, content) = match self.view {
                // TODO: Too big & plain. looks silly
                View::Help => (
                    "Help",
                    Paragraph::new(HELP_TEXT.iter().map(|&s| Line::from(s)).collect::<Vec<_>>()),
                ),
                View::Log => ("Big Log", Paragraph::new("Placeholder for Big Log")),
                View::Library => ("Library", Paragraph::new("Placeholder for Library")),
                View::Main => unreachable!(),
            };
            draw_modal(area, buf, title, content);
        }
    }
}

/// Helper function to render a centered modal popup
fn draw_modal(area: Rect, buf: &mut Buffer, title: &str, content: impl Widget) {
    let area = popup_area(area, 60, 60);
    ratatui::widgets::Clear.render(area, buf);

    let block = Block::bordered()
        .title(format!(" {} ", title))
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Double);

    let inner = block.inner(area);
    block.render(area, buf);
    content.render(inner, buf);
}

/// Renders the bottom bar controls based on app state
fn controls<I: BeatInput>(state: &'_ App<I>) -> Line<'_> {
    // This feels gross, but it works
    let mut parts = Vec::new();

    if state.view != View::Main {
        parts.push(" <Esc>: Main | <F1>: Help");
    } else {
        parts.push(" <F1>: Help");
    }

    parts.push("<F2>: Log");
    parts.push("<F3>: Quit");

    parts.push(if state.paused {
        "<F4>: Play"
    } else {
        "<F4>: Pause"
    });

    parts.push("<F5>: Library ");
    Line::from(parts.join(" | ")).centered()
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
///
/// yoinked from <https://ratatui.rs/examples/apps/popup/>
fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
