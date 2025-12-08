use crate::{App, audio::StreamStatus};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut control_str: String = " <F3>: Quit | <P>: ".to_owned();
        match self.paused {
            true => control_str.push_str("Play "),
            false => control_str.push_str("Pause "),
        };

        let main_block = Block::bordered()
            .title(" bytebeat   ")
            .title_alignment(Alignment::Left)
            .border_type(BorderType::Rounded)
            .title_bottom(control_str);

        let main_interior = Layout::default()
            .direction(Direction::Vertical)
            // One big widget area, and a little bottom bar
            .constraints(vec![Constraint::Percentage(100), Constraint::Min(2)])
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
        // Dummy text (for now)
        Paragraph::new("Test text, please ignore.")
            .centered()
            .render(main_interior[0], buf);
        // Status bar text must be rendered before status bar
        Paragraph::new(stream_status)
            .centered()
            .style(Style::default().add_modifier(Modifier::BOLD))
            .render(status_block.inner(main_interior[1]), buf);
        status_block.render(main_interior[1], buf);
    }
}
