#[cfg(feature = "tui")]
use std::{io, time::Duration};

#[cfg(feature = "tui")]
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
#[cfg(feature = "tui")]
use ratatui::{
    prelude::{CrosstermBackend, Terminal},
    widgets::{Block, Borders, Paragraph},
};

#[cfg(feature = "tui")]
use crate::queue::SqliteQueue;

#[cfg(feature = "tui")]
pub fn run_dashboard(queue: &SqliteQueue) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|frame| {
            let applied = queue
                .list(crate::models::QueueStatus::Applied)
                .map(|records| records.len())
                .unwrap_or(0);
            let pending = queue
                .list(crate::models::QueueStatus::Pending)
                .map(|records| records.len())
                .unwrap_or(0);
            let failed = queue
                .list(crate::models::QueueStatus::Failed)
                .map(|records| records.len())
                .unwrap_or(0);
            let in_progress = queue
                .list(crate::models::QueueStatus::InProgress)
                .map(|records| records.len())
                .unwrap_or(0);

            let text = format!(
                "Queue Status\n\nPending: {pending}\nIn Progress: {in_progress}\nApplied: {applied}\nFailed: {failed}\n\nPress q to exit.",
            );
            let block = Paragraph::new(text).block(Block::default().title("Hyperion Dashboard").borders(Borders::ALL));
            frame.render_widget(block, frame.size());
        })?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(not(feature = "tui"))]
pub fn run_dashboard(_queue: &crate::queue::SqliteQueue) -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "tui feature not enabled; rebuild with --features tui"
    ))
}
