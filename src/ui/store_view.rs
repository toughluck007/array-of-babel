use crate::app::App;
use crate::sim::game::Game;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

pub fn render(frame: &mut Frame, app: &App, game: &Game) {
    let area = centered_rect(60, 70, frame.size());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title("Array Exchange")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(inner);

    let mut items: Vec<ListItem> = Vec::new();
    for (idx, item) in game.store_items().iter().enumerate() {
        let cost_opt = game.item_cost(idx);
        let purchased = game.store_purchases(idx).unwrap_or(0);
        let affordable = cost_opt
            .map(|cost| game.state.credits >= cost)
            .unwrap_or(false);
        let mut line = Vec::new();
        let name_style = Style::default()
            .fg(if affordable {
                Color::Yellow
            } else if cost_opt.is_some() {
                Color::DarkGray
            } else {
                Color::Gray
            })
            .add_modifier(Modifier::BOLD);
        line.push(Span::styled(item.name, name_style));
        match cost_opt {
            Some(cost) => line.push(Span::raw(format!("  [{} cr]", cost))),
            None => line.push(Span::styled(
                "  [SOLD OUT]",
                Style::default().fg(Color::DarkGray),
            )),
        }
        if purchased > 0 {
            if let Some(max) = item.max_purchases {
                line.push(Span::raw(format!("  (owned {purchased}/{max})")));
            } else {
                line.push(Span::raw(format!("  (owned {purchased})")));
            }
        } else if let Some(max) = item.max_purchases {
            line.push(Span::raw(format!("  (limit {max})")));
        }
        let detail = Line::from(item.description);
        let list_item = ListItem::new(vec![Line::from(line), detail]);
        items.push(list_item);
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Upgrades"))
        .highlight_symbol("▶ ")
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White));
    let mut state = ListState::default();
    if !game.store_items().is_empty() {
        let selection = app.selected_store_item.min(game.store_items().len() - 1);
        state.select(Some(selection));
    }
    frame.render_stateful_widget(list, layout[0], &mut state);

    let footer = Paragraph::new(vec![Line::from(vec![
        Span::raw(format!("Credits: {}", game.state.credits)),
        Span::raw("  •  Enter to purchase  •  Esc/S to close"),
    ])])
    .wrap(Wrap { trim: true });
    frame.render_widget(footer, layout[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    let vertical = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1]);

    vertical[1]
}
