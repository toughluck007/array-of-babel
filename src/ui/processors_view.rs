use crate::app::{App, FocusTarget};
use crate::sim::game::Game;
use crate::sim::processors::ProcessorStatus;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn render(frame: &mut Frame, area: Rect, app: &App, game: &Game) {
    let highlight = app.focus() == FocusTarget::Processors;
    let border_style = if highlight {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let mut items: Vec<ListItem> = Vec::new();
    for processor in &game.state.processors {
        let header = Line::from(vec![
            Span::styled(
                processor.name.clone(),
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::raw(format!("| speed {:.2}", processor.speed)),
            Span::raw(" "),
            Span::raw(format!("| bias {:+}", processor.quality_bias)),
        ]);

        let status_line = match &processor.status {
            ProcessorStatus::Idle => Line::from(vec![Span::styled(
                "Idle",
                Style::default().fg(Color::Green),
            )]),
            ProcessorStatus::Busy { job, .. } => {
                let (remaining, total) = processor.remaining_and_total().unwrap_or((0, 1));
                let elapsed = total.saturating_sub(remaining);
                let remaining_secs = remaining as f64 / 1000.0;
                let total_secs = total as f64 / 1000.0;
                let elapsed_secs = elapsed as f64 / 1000.0;
                let progress = if total > 0 {
                    (elapsed as f64 / total as f64).min(1.0)
                } else {
                    0.0
                };
                let progress_pct = (progress * 100.0).round() as i32;
                Line::from(vec![
                    Span::styled(
                        format!("Working on {}", job.name),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::raw(" "),
                    Span::raw(format!(
                        "{elapsed_secs:.1}/{total_secs:.1}s ({progress_pct}%)"
                    )),
                    Span::raw(" "),
                    Span::raw(format!("remaining {remaining_secs:.1}s")),
                ])
            }
        };

        items.push(ListItem::new(vec![header, status_line]));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title("Processors")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if !game.state.processors.is_empty() {
        let selection = app.selected_processor.min(game.state.processors.len() - 1);
        state.select(Some(selection));
    }
    frame.render_stateful_widget(list, area, &mut state);
}
