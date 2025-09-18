use crate::app::App;
use crate::sim::economy;
use crate::sim::game::{DAEMON_UNLOCK_CREDITS, Game};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

pub fn render(frame: &mut Frame, area: Rect, app: &App, game: &Game) {
    let sections = Layout::vertical([Constraint::Length(9), Constraint::Min(3)]).split(area);

    let stats_block = Block::default().title("Systems").borders(Borders::ALL);
    let stats_area = stats_block.inner(sections[0]);
    frame.render_widget(stats_block, sections[0]);

    let storage = &game.state.storage;
    let passive_preview = economy::passive_income(storage.stored);
    let spawn_pct = (game.job_spawn_progress() * 100.0).min(100.0);
    let day_pct = (game.day_progress() * 100.0).min(100.0);
    let daemon_status = if !game.state.daemon_unlocked {
        format!("Locked ({} cr needed)", DAEMON_UNLOCK_CREDITS)
    } else if game.state.daemon_enabled {
        "Enabled".to_string()
    } else {
        "Disabled".to_string()
    };
    let pending_job = app
        .pending_job
        .as_ref()
        .map(|job| job.name.as_str())
        .unwrap_or("None");

    let stats_lines = vec![
        Line::from(vec![
            Span::styled("Credits", Style::default().fg(Color::Yellow)),
            Span::raw(format!(": {}", game.state.credits)),
            Span::raw("    Upkeep/day: "),
            Span::raw(format!("{}", game.total_upkeep())),
        ]),
        Line::from(vec![
            Span::styled("Storage", Style::default().fg(Color::LightGreen)),
            Span::raw(format!(
                ": {}/{} (free {} units)",
                storage.stored,
                storage.capacity,
                storage.free_capacity()
            )),
        ]),
        Line::from(vec![
            Span::raw("Passive income each cycle: "),
            Span::raw(format!("{} credits", passive_preview)),
        ]),
        Line::from(vec![
            Span::raw("Instruction tags: "),
            Span::styled(
                game.state.unlocked_tags.join(", "),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::raw("Daemon status: "),
            Span::styled(daemon_status, Style::default().fg(Color::Magenta)),
        ]),
        Line::from(vec![
            Span::raw("Job spawn timer: "),
            Span::raw(format!("{spawn_pct:.0}%")),
            Span::raw("    Day progress: "),
            Span::raw(format!("{day_pct:.0}%")),
        ]),
        Line::from(vec![
            Span::raw("Pending job: "),
            Span::styled(pending_job.to_string(), Style::default().fg(Color::Cyan)),
        ]),
    ];

    let paragraph = Paragraph::new(stats_lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, stats_area);

    let log_block = Block::default().title("Event Log").borders(Borders::ALL);
    let log_area = log_block.inner(sections[1]);
    frame.render_widget(log_block, sections[1]);

    let mut items: Vec<ListItem> = game
        .messages()
        .map(|msg| ListItem::new(msg.clone()))
        .collect();
    if items.is_empty() {
        items.push(ListItem::new("No events yet. Stay vigilant."));
    }
    frame.render_widget(List::new(items), log_area);
}
