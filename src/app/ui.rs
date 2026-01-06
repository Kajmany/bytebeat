use crate::{
    App,
    app::{View, input::BeatInput, volume},
    audio::StreamStatus,
};

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

/// Used in calculation for [`crate::app::BeatInput::height_hint`]
/// May show another line for more errors
/// Not everything wrong becomes a discrete error, it's actually hard to rack up this many
pub const MAX_ERRORS_SHOWN: usize = 3;

/// Scientists estimate decades until average TUI dev rediscovers i18n
const HELP_TEXT: &[&str] = &[
    "Controls:",
    "  Esc: Close Help or return to Main",
    "    - (May also repeat key to close Help/View)",
    "  F1: Help",
    "  F2: Log",
    "  F3: Quit",
    "  F4: Play/Pause",
    "  F5: Library",
    "  F6: About",
    "  Up/Down: Volume",
    "",
    "Interactive Input:",
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

        // BigLog and Library views replace the scope and log areas
        let display_constraints = match self.view {
            View::BigLog | View::Library => vec![
                // BigLog | Library
                Constraint::Percentage(95),
                // Input
                Constraint::Length(self.beat_input.height_hint()),
                // Status bar
                Constraint::Length(3),
            ],
            _ => vec![
                // Scope
                Constraint::Percentage(80),
                // Logs
                Constraint::Percentage(15),
                // Input
                Constraint::Length(self.beat_input.height_hint()),
                // Status bar
                Constraint::Length(3),
            ],
        };

        let main_interior = Layout::default()
            .direction(Direction::Vertical)
            .constraints(display_constraints)
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

        // Render view-dependent content in the top area(s)
        match self.view {
            View::BigLog => {
                Paragraph::new("Placeholder for Big Log")
                    .block(Block::bordered().title(" Big Log "))
                    .render(main_interior[0], buf);
            }
            View::Library => {
                Paragraph::new("Placeholder for Library")
                    .block(Block::bordered().title(" Library "))
                    .render(main_interior[0], buf);
            }
            View::Main => {
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
            }
        }

        // Input and status bar indices shift based on view layout
        let (input_idx, status_idx) = match self.view {
            View::BigLog | View::Library => (1, 2),
            View::Main => (2, 3),
        };

        self.beat_input.render(main_interior[input_idx], buf);

        let status_area = status_block.inner(main_interior[status_idx]);
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
        status_block.render(main_interior[status_idx], buf);

        if self.show_help {
            let content =
                Paragraph::new(HELP_TEXT.iter().map(|&s| Line::from(s)).collect::<Vec<_>>());
            draw_modal(area, buf, "Help", content);
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
    // Messy. ensures forward and back padding
    // Active is highlighted, maybe it should be other way around?
    let sep = Span::raw(" | ");
    let active = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let mut spans = vec![Span::raw(" ")]; // Leading padding

    if state.view != View::Main || state.show_help {
        spans.push(Span::raw("<Esc>: Back"));
        spans.push(sep.clone());
    }

    let help_span = Span::styled(
        "<F1>: Help",
        if state.show_help {
            active
        } else {
            Style::default()
        },
    );
    spans.push(help_span);
    spans.push(sep.clone());

    let log_span = Span::styled(
        "<F2>: Log",
        if state.view == View::BigLog {
            active
        } else {
            Style::default()
        },
    );
    spans.push(log_span);
    spans.push(sep.clone());

    spans.push(Span::raw("<F3>: Quit"));
    spans.push(sep.clone());

    let play_text = if state.paused {
        "<F4>: Play"
    } else {
        "<F4>: Pause"
    };
    spans.push(Span::raw(play_text));
    spans.push(sep.clone());

    let lib_span = Span::styled(
        "<F5>: Library",
        if state.view == View::Library {
            active
        } else {
            Style::default()
        },
    );
    spans.push(lib_span);
    spans.push(Span::raw(" ")); // Trailing padding

    Line::from(spans).centered()
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
