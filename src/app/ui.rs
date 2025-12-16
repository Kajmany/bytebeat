use crate::{App, app::volume, audio::StreamStatus};

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

/// Used in calculation for [`crate::app::BeatInput::height_hint`]
/// May show another line for more errors
/// Not everything wrong becomes a discrete error, it's actually hard to rack up this many
pub const MAX_ERRORS_SHOWN: usize = 3;

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // This could be less ugly. It'll do for now.
        let mut control_str: String = " <F3>: Quit | ".to_owned();
        match self.paused {
            true => control_str.push_str("<F4>: Play | "),
            false => control_str.push_str("<F4>: Pause | "),
        };
        control_str.push_str("<Enter>: Ship! | <Backspace>: Delete ");

        let main_block = Block::bordered()
            .title(" bytebeat   ")
            .title_alignment(Alignment::Left)
            .border_type(BorderType::Rounded)
            .title_bottom(control_str);

        let main_interior = Layout::default()
            .direction(Direction::Vertical)
            // One big widget area, and a little bottom bar
            .constraints(vec![
                // Scope
                Constraint::Percentage(75),
                // Logs
                Constraint::Percentage(15),
                // Input
                Constraint::Length(self.beat_input.height_hint()),
                // Status bar
                Constraint::Max(3),
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
    }
}
