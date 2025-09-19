mod app;
mod persist;
mod sim;
mod ui;

use anyhow::Result;
use app::{App, FocusTarget};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{execute, terminal};
use persist::{load_game, save_game};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use sim::game::Game;
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task;

#[tokio::main]
async fn main() -> Result<()> {
    let loaded = load_game()?;
    let mut game = match loaded {
        Some(state) => {
            let mut game = Game::from_state(state);
            game.add_message("Loaded save state.");
            game
        }
        None => {
            let mut game = Game::fresh();
            game.add_message("Welcome to the Array of Babel.");
            game
        }
    };

    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal, &mut game).await;
    restore_terminal(&mut terminal)?;

    save_game(&game.state)?;
    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, game: &mut Game) -> Result<()> {
    let mut app = App::new();
    app.clamp_job_selection(game.state.jobs.len());
    app.clamp_processor_selection(game.state.processors.len());
    app.clamp_store_selection(game.store_items().len());

    let (input_tx, mut input_rx) = mpsc::unbounded_channel();
    task::spawn(async move {
        loop {
            match task::spawn_blocking(crossterm::event::read).await {
                Ok(Ok(event)) => {
                    if input_tx.send(event).is_err() {
                        break;
                    }
                }
                Ok(Err(_)) => break,
                Err(_) => break,
            }
        }
    });

    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(100);
    let mut should_quit = false;

    loop {
        terminal.draw(|f| ui::render(f, &app, game))?;
        if should_quit {
            break;
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        tokio::select! {
            Some(event) = input_rx.recv() => {
                if handle_event(event, &mut app, game)? {
                    should_quit = true;
                }
            }
            _ = tokio::time::sleep(timeout) => {
                let delta = last_tick.elapsed();
                last_tick = Instant::now();
                game.update(delta);
                app.clamp_job_selection(game.state.jobs.len());
                app.clamp_processor_selection(game.state.processors.len());
            }
        }
    }

    Ok(())
}

fn handle_event(event: Event, app: &mut App, game: &mut Game) -> Result<bool> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => handle_key_event(key, app, game),
        Event::Resize(_, _) => Ok(false),
        _ => Ok(false),
    }
}

fn handle_key_event(key: KeyEvent, app: &mut App, game: &mut Game) -> Result<bool> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if key.code == KeyCode::Char('c') {
            return Ok(true);
        }
    }

    if app.store_open {
        return handle_store_key(key, app, game);
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Ok(true),
        KeyCode::Esc => {
            if let Some(job) = app.pending_job.take() {
                game.return_job(job);
                app.clamp_job_selection(game.state.jobs.len());
            }
            Ok(false)
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            app.toggle_store();
            Ok(false)
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if app.focus() == FocusTarget::Processors {
                if game.state.processors.is_empty() {
                    game.add_message("No processors available.");
                } else {
                    let index = app.selected_processor.min(game.state.processors.len() - 1);
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        game.toggle_honor_cooling(index);
                    } else {
                        game.cycle_daemon_mode(index);
                    }
                }
            } else {
                game.add_message("Focus a processor to adjust automation.");
            }
            Ok(false)
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if app.focus() == FocusTarget::Processors {
                if game.state.processors.is_empty() {
                    game.add_message("No processors available to replace.");
                } else {
                    let index = app.selected_processor.min(game.state.processors.len() - 1);
                    let result = if key.modifiers.contains(KeyModifiers::SHIFT) {
                        game.replace_model_direct(index)
                    } else {
                        game.replace_processor_direct(index)
                    };
                    if let Err(err) = result {
                        game.add_message(format!("Replacement failed: {err}"));
                    }
                }
            } else {
                game.add_message("Focus a processor to replace hardware.");
            }
            Ok(false)
        }
        KeyCode::Tab => {
            app.next_focus();
            Ok(false)
        }
        KeyCode::BackTab => {
            app.next_focus();
            Ok(false)
        }
        KeyCode::Left => {
            app.set_focus(FocusTarget::Processors);
            Ok(false)
        }
        KeyCode::Right => {
            app.set_focus(FocusTarget::Jobs);
            Ok(false)
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
            move_selection(app, game, -1);
            Ok(false)
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
            move_selection(app, game, 1);
            Ok(false)
        }
        KeyCode::Enter => handle_enter(app, game),
        KeyCode::Char('a') | KeyCode::Char('A') => handle_enter(app, game),
        _ => Ok(false),
    }
}

fn move_selection(app: &mut App, game: &Game, delta: isize) {
    match app.focus() {
        FocusTarget::Jobs => {
            let len = game.state.jobs.len();
            if len > 0 {
                let mut idx = app.selected_job as isize + delta;
                if idx < 0 {
                    idx = len as isize - 1;
                } else if idx >= len as isize {
                    idx = 0;
                }
                app.selected_job = idx as usize;
            }
        }
        FocusTarget::Processors => {
            let len = game.state.processors.len();
            if len > 0 {
                let mut idx = app.selected_processor as isize + delta;
                if idx < 0 {
                    idx = len as isize - 1;
                } else if idx >= len as isize {
                    idx = 0;
                }
                app.selected_processor = idx as usize;
            }
        }
    }
}

fn handle_enter(app: &mut App, game: &mut Game) -> Result<bool> {
    match app.focus() {
        FocusTarget::Jobs => {
            if app.pending_job.is_some() {
                game.add_message("A job is already awaiting assignment.");
                return Ok(false);
            }
            if let Some(job) = game.take_job(app.selected_job) {
                let name = job.name.clone();
                app.pending_job = Some(job);
                app.clamp_job_selection(game.state.jobs.len());
                game.add_message(format!("{name} queued for assignment."));
            } else {
                game.add_message("No jobs available to queue.");
            }
            Ok(false)
        }
        FocusTarget::Processors => {
            if game.state.processors.is_empty() {
                game.add_message("No processors available.");
                return Ok(false);
            }
            let idx = app
                .selected_processor
                .min(game.state.processors.len().saturating_sub(1));
            if let Some(job) = app.pending_job.take() {
                let job_clone = job.clone();
                match game.assign_job_to_processor(job_clone, idx, false) {
                    Ok(_) => Ok(false),
                    Err(err) => {
                        game.add_message(format!("Assignment failed: {err}"));
                        app.pending_job = Some(job);
                        Ok(false)
                    }
                }
            } else {
                if game.accept_assist_suggestion(idx) {
                    app.clamp_job_selection(game.state.jobs.len());
                }
                Ok(false)
            }
        }
    }
}

fn handle_store_key(key: KeyEvent, app: &mut App, game: &mut Game) -> Result<bool> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('s') | KeyCode::Char('S') => {
            app.toggle_store();
            Ok(false)
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
            if app.selected_store_item > 0 {
                app.selected_store_item -= 1;
            }
            Ok(false)
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
            if app.selected_store_item + 1 < game.store_items().len() {
                app.selected_store_item += 1;
            }
            Ok(false)
        }
        KeyCode::Enter => {
            let processor_index = if game.state.processors.is_empty() {
                None
            } else {
                Some(app.selected_processor.min(game.state.processors.len() - 1))
            };
            if let Err(err) = game.purchase_item(app.selected_store_item, processor_index) {
                game.add_message(format!("Purchase failed: {err}"));
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
