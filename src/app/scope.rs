//! Scope widget for visualizing audio samples.
//!
//! Uses some state from the audio thread to render a chart. Spoiler: It's just a ring buffer and a
//! ratatui chart plumbed together.
//!
//! FIXME: AI slopped the buffer logic and it's needlessly complicated and probably inefficient
use std::collections::VecDeque;
use std::sync::atomic::{AtomicI32, Ordering};

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    symbols::Marker,
    widgets::{Axis, Block, BorderType, Chart, Dataset, GraphType, Widget},
};

/// How many samples we display on one chart
const CHART_SAMPLES: usize = 32000;

pub struct Scope {
    /// Samples stream incoming from audio thread
    consumer: rtrb::Consumer<u8>,
    /// Highest sample we've read off the ringbuffer
    t_read: i32,
    /// Which sample is about to be played, set by audio thread
    t_play: &'static AtomicI32,
    /// Intermediate queue waiting to be displayed
    intermediate_queue: VecDeque<u8>,
    /// The slice we give to the chart
    chart_buffer: VecDeque<(f64, f64)>,
    /// The 't' of the last sample pushed to the chart_buffer
    t_chart_head: i32,
}

impl Scope {
    pub fn new(consumer: rtrb::Consumer<u8>, t_play: &'static AtomicI32) -> Self {
        Self {
            consumer,
            t_read: 0,
            t_play,
            intermediate_queue: VecDeque::with_capacity(4096),
            chart_buffer: VecDeque::with_capacity(CHART_SAMPLES),
            t_chart_head: -1,
        }
    }

    pub fn update(&mut self) {
        // Pop all available samples
        // TODO: This could be done with chunks, maybe faster. probably doesn't matter
        while let Ok(sample) = self.consumer.pop() {
            self.intermediate_queue.push_back(sample);
            self.t_read += 1;
        }

        // Read t_play logic to decide how many elements go inside of the chart_buffer
        let play_head = self.t_play.load(Ordering::Relaxed);

        // We assume we want to sync the chart to the playback head.
        // If play_head is ahead of what we've pushed to chart, push more.
        if play_head > self.t_chart_head {
            let needed = (play_head - self.t_chart_head) as usize;
            let available = self.intermediate_queue.len();
            // We can only push what we have
            let count = needed.min(available);

            for _ in 0..count {
                if let Some(sample) = self.intermediate_queue.pop_front() {
                    self.t_chart_head += 1;
                    self.chart_buffer
                        .push_back((self.t_chart_head as f64, sample as f64));
                    if self.chart_buffer.len() > CHART_SAMPLES {
                        self.chart_buffer.pop_front();
                    }
                }
            }
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        // Optimize: use make_contiguous to get a slice for the Chart without allocation
        self.chart_buffer.make_contiguous();
        let (data, _) = self.chart_buffer.as_slices();

        let latest_t = self.t_chart_head;

        // Fixed window scrolling logic
        // We want to fill from left to right, then scroll
        let window = CHART_SAMPLES as f64;
        let end_x = if latest_t < (CHART_SAMPLES as i32) {
            window
        } else {
            latest_t as f64
        };
        let start_x = end_x - window;

        let datasets = vec![
            Dataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Scatter)
                .style(Style::default().fg(Color::Cyan))
                .data(data),
        ];

        let chart = Chart::new(datasets)
            .block(
                Block::bordered()
                    .title(format!(" Scope - t: {} ", latest_t))
                    .border_type(BorderType::Rounded),
            )
            .x_axis(
                Axis::default()
                    .style(Style::default().fg(Color::Gray))
                    .bounds([start_x, end_x]),
            )
            .y_axis(
                Axis::default()
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, 255.0]),
            );
        chart.render(area, buf);
    }
}
