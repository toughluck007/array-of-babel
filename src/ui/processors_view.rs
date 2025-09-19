use crate::app::{App, FocusTarget};
use crate::sim::game::{AssistSuggestion, Game};
use crate::sim::processors::{DaemonMode, ProcessorStatus};
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
    for (index, processor) in game.state.processors.iter().enumerate() {
        let reliability_pct = processor.reliability_display() * 100.0;
        let reliability_style = if reliability_pct >= 90.0 {
            Style::default().fg(Color::LightGreen)
        } else if reliability_pct >= 70.0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::LightRed)
        };
        let automation_label = match processor.daemon_mode {
            DaemonMode::Off => "Off",
            DaemonMode::Assist => "Assist",
            DaemonMode::Auto => "Auto",
        };
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
            Span::raw(" "),
            Span::raw(format!("| auto {}", automation_label)),
            Span::raw(" "),
            Span::styled(format!("| rel {reliability_pct:.1}%"), reliability_style),
        ]);

        let wear_pct = (processor.wear * 100.0).min(100.0);
        let power_draw = processor.last_power_draw();

        let status_line = match &processor.status {
            ProcessorStatus::Idle => Line::from(vec![
                Span::styled("Idle", Style::default().fg(Color::Green)),
                Span::raw("  •  cooling "),
                Span::raw(format!(
                    "{}/{}",
                    processor.cooling_level,
                    processor.cooling_cap()
                )),
                Span::raw("  •  hardening "),
                Span::raw(format!("{}", processor.hardening_level)),
                Span::raw("  •  wear "),
                Span::raw(format!("{wear_pct:.0}%")),
                Span::raw("  •  draw "),
                Span::raw(format!("{power_draw:.1} kWh")),
            ]),
            ProcessorStatus::Working(work) => {
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
                let heat = processor.heat_display();
                let heat_span = if work.overheating {
                    Span::styled(
                        format!("heat {heat:.2}"),
                        Style::default().fg(Color::LightRed),
                    )
                } else {
                    Span::raw(format!("heat {heat:.2}"))
                };
                Line::from(vec![
                    Span::styled(
                        format!("Working on {}", work.job.name),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::raw(" "),
                    Span::raw(format!(
                        "{elapsed_secs:.1}/{total_secs:.1}s ({progress_pct}%)"
                    )),
                    Span::raw(" "),
                    Span::raw(format!("remaining {remaining_secs:.1}s")),
                    Span::raw("  •  "),
                    heat_span,
                    Span::raw("  •  draw "),
                    Span::raw(format!("{power_draw:.1} kWh")),
                ])
            }
            ProcessorStatus::BurntOut => Line::from(vec![Span::styled(
                "Burnt Out — press [R] to replace",
                Style::default().fg(Color::LightRed),
            )]),
            ProcessorStatus::Destroyed => Line::from(vec![Span::styled(
                "Destroyed — replace required",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]),
        };

        let mut lines = vec![header, status_line];
        if matches!(processor.daemon_mode, DaemonMode::Assist) {
            if let Some(AssistSuggestion {
                job_index,
                eta_secs,
                reliability,
                heat,
            }) = game.assist_suggestion(index)
            {
                if let Some(job) = game.state.jobs.get(job_index) {
                    lines.push(Line::from(vec![
                        Span::styled("Assist", Style::default().fg(Color::LightBlue)),
                        Span::raw(format!(
                            ": {} ({eta_secs:.1}s, rel {:.0}%, heat {:.2})",
                            job.name,
                            reliability * 100.0,
                            heat
                        )),
                    ]));
                }
            }
        }

        items.push(ListItem::new(lines));
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
