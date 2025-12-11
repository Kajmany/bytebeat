use crate::{App, audio::StreamStatus};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

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
                Constraint::Percentage(40),
                Constraint::Percentage(40),
                Constraint::Percentage(18),
                Constraint::Min(2),
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
        // Dummy text (for now) TODO: Wave form widget!
        Paragraph::new("Pretend this is a wave form visualizer. Trippy!")
            .centered()
            .render(main_interior[0], buf);

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
        Paragraph::new(stream_status)
            .centered()
            .style(Style::default().add_modifier(Modifier::BOLD))
            .render(status_block.inner(main_interior[3]), buf);
        status_block.render(main_interior[3], buf);
    }
}
