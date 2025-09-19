use crate::app::App;
use crate::sim::game::Game;
use crate::sim::processors::DaemonMode;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub mod jobs_view;
pub mod processors_view;
pub mod storage_view;
pub mod store_view;

pub fn render(frame: &mut Frame, app: &App, game: &Game) {
    let size = frame.size();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(size);

    render_header(frame, layout[0], app, game);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(layout[1]);

    processors_view::render(frame, columns[0], app, game);
    jobs_view::render(frame, columns[1], app, game);
    storage_view::render(frame, columns[2], app, game);

    render_footer(frame, layout[2]);

    if app.store_open {
        store_view::render(frame, app, game);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App, game: &Game) {
    let pending = app
        .pending_job
        .as_ref()
        .map(|job| job.name.as_str())
        .unwrap_or("None");
    let automation_summary = if !game.state.daemon_unlocked {
        "Locked".to_string()
    } else {
        let auto = game
            .state
            .processors
            .iter()
            .filter(|p| p.daemon_mode == DaemonMode::Auto)
            .count();
        let assist = game
            .state
            .processors
            .iter()
            .filter(|p| p.daemon_mode == DaemonMode::Assist)
            .count();
        format!("{auto} auto / {assist} assist")
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                "Array of Babel",
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  •  Credits: "),
            Span::styled(
                format!("{}", game.state.credits),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  •  Pending: "),
            Span::styled(pending.to_string(), Style::default().fg(Color::Cyan)),
            Span::raw("  •  Automation: "),
            Span::styled(automation_summary, Style::default().fg(Color::Magenta)),
        ]),
        Line::from(vec![Span::raw(
            "Use Tab to shift focus, Enter to interact with the highlighted panel.",
        )]),
    ];

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(paragraph, area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let instructions = Paragraph::new(Line::from(vec![
        Span::raw("Hotkeys: "),
        Span::styled("[J/K]", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate  •  "),
        Span::styled("[Tab]", Style::default().fg(Color::Yellow)),
        Span::raw(" switch focus  •  "),
        Span::styled("[Enter]", Style::default().fg(Color::Yellow)),
        Span::raw(" take/assign  •  "),
        Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
        Span::raw(" cancel pending  •  "),
        Span::styled("[S]", Style::default().fg(Color::Yellow)),
        Span::raw(" store  •  "),
        Span::styled("[D]", Style::default().fg(Color::Yellow)),
        Span::raw(" cycle automation  •  "),
        Span::styled("[Shift+D]", Style::default().fg(Color::Yellow)),
        Span::raw(" cooling safety  •  "),
        Span::styled("[R]", Style::default().fg(Color::Yellow)),
        Span::raw(" replace unit  •  "),
        Span::styled("[Shift+R]", Style::default().fg(Color::Yellow)),
        Span::raw(" replace model  •  "),
        Span::styled("[Q]", Style::default().fg(Color::Yellow)),
        Span::raw(" save & quit"),
    ]))
    .wrap(Wrap { trim: true })
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(instructions, area);
}
