use crate::app::{App, FocusTarget};
use crate::sim::game::Game;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn render(frame: &mut Frame, area: Rect, app: &App, game: &Game) {
    let highlight = app.focus() == FocusTarget::Jobs;
    let border_style = if highlight {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let mut items: Vec<ListItem> = game
        .state
        .jobs
        .iter()
        .map(|job| {
            let time_secs = job.base_time_ms as f64 / 1000.0;
            let line = Line::from(vec![
                Span::styled(job.name.clone(), Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::raw(format!("| {} cr", job.base_reward)),
                Span::raw(" "),
                Span::raw(format!("| {:.1}s", time_secs)),
                Span::raw(" "),
                Span::raw(format!("| Q{}", job.quality_target)),
            ]);
            let detail = Line::from(vec![Span::raw(format!(
                "Tag: {} • Data yield: {} units",
                job.tag, job.data_output
            ))]);
            ListItem::new(vec![line, detail])
        })
        .collect();

    if items.is_empty() {
        items.push(ListItem::new(Line::from(vec![Span::raw(
            "No jobs waiting.",
        )])));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title("Job Board")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if !game.state.jobs.is_empty() {
        let selection = app.selected_job.min(game.state.jobs.len() - 1);
        state.select(Some(selection));
    }
    frame.render_stateful_widget(list, area, &mut state);
}
