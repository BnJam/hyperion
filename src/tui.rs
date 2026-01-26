#[cfg(feature = "tui")]
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(feature = "tui")]
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[cfg(feature = "tui")]
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{CrosstermBackend, Terminal},
    widgets::{Block, Borders, Paragraph, Row, Table},
};

#[cfg(feature = "tui")]
use crate::models::QueueStatus;
#[cfg(feature = "tui")]
use crate::queue::SqliteQueue;

#[cfg(feature = "tui")]
pub struct TuiConfig {
    pub db_path: String,
    pub worker_count: usize,
    pub agent_count: usize,
    pub modified_files: Arc<Mutex<VecDeque<String>>>,
}

#[cfg(feature = "tui")]
pub fn run_dashboard(queue: &SqliteQueue) -> anyhow::Result<()> {
    run_dashboard_with_config(
        queue,
        TuiConfig {
            db_path: "hyperion.db".to_string(),
            worker_count: 0,
            agent_count: 0,
        },
    )
}

#[cfg(feature = "tui")]
pub fn run_dashboard_with_config(queue: &SqliteQueue, config: TuiConfig) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|frame| {
            let area = frame.size();
            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Length(12),
                    Constraint::Min(12),
                ])
                .split(area);

            let status_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                ])
                .split(vertical_chunks[0]);

            let pending_records = queue.list(QueueStatus::Pending).unwrap_or_default();
            let in_progress_records = queue
                .list(QueueStatus::InProgress)
                .unwrap_or_default();
            let applied_count = queue
                .list(QueueStatus::Applied)
                .map(|records| records.len())
                .unwrap_or(0);
            let failed_count = queue
                .list(QueueStatus::Failed)
                .map(|records| records.len())
                .unwrap_or(0);
            let dead_letters = queue.dead_letter_count().unwrap_or(0);

            let summary_text = format!(
                "Queue Overview\nPending: {}\nIn Progress: {}\nApplied: {}\nFailed: {}\nDead Letters: {}",
                pending_records.len(),
                in_progress_records.len(),
                applied_count,
                failed_count,
                dead_letters
            );
            let summary = Paragraph::new(summary_text)
                .block(Block::default().title("Queue").borders(Borders::ALL));
            frame.render_widget(summary, status_chunks[0]);

            let session_text = match queue.latest_agent_session() {
                Ok(Some(session)) => format!(
                    "Session: {} (allow_all_tools={}, last_used={})",
                    session.resume_id, session.allow_all_tools, session.last_used
                ),
                _ => "Session: <none>".to_string(),
            };
            let runtime_text = format!(
                "Runtime Insights\nDB: {}\nWorkers: {}\nAgents: {}\n{}",
                config.db_path, config.worker_count, config.agent_count, session_text
            );
            let runtime = Paragraph::new(runtime_text)
                .block(Block::default().title("Runtime").borders(Borders::ALL));
            frame.render_widget(runtime, status_chunks[1]);

            let guidance_text = "Controls\nq : Quit\nhyperion request <file> : enqueue task request\nhyperion session init --resume=<token> [--model=<name>] [--allow-all-tools=<bool>]\nhyperion session list : view stored Copilot sessions\nhyperion Tui : refresh dashboard"
                .to_string();
            let guidance = Paragraph::new(guidance_text)
                .block(Block::default().title("Guidance").borders(Borders::ALL));
            frame.render_widget(guidance, status_chunks[2]);

            let queue_middle_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(vertical_chunks[1]);

            let mut queue_records = Vec::new();
            queue_records.extend(pending_records.clone());
            queue_records.extend(in_progress_records.clone());

            let queue_rows: Vec<Row> = queue_records
                .iter()
                .take(10)
                .map(|record| {
                    Row::new(vec![
                        record.id.to_string(),
                        record.payload.task_id.clone(),
                        record.payload.agent.clone(),
                        record.status.as_str().to_string(),
                        record.attempts.to_string(),
                    ])
                })
                .collect();

            let queue_widths = [
                Constraint::Length(6),
                Constraint::Length(20),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
            ];
            let queue_table = Table::new(queue_rows, queue_widths)
                .header(Row::new(vec!["ID", "Task ID", "Agent", "Status", "Attempts"]))
                .block(
                    Block::default()
                        .title("Recent Queue Entries")
                        .borders(Borders::ALL),
                );
            frame.render_widget(queue_table, queue_middle_chunks[0]);

            let history_records = queue.recent_records(100).unwrap_or_default();
            let history_rows: Vec<Row> = history_records
                .iter()
                .take(8)
                .map(|record| {
                    Row::new(vec![
                        record.payload.task_id.clone(),
                        record.payload.agent.clone(),
                        record.status.as_str().to_string(),
                        record.attempts.to_string(),
                    ])
                })
                .collect();

            let history_widths = [
                Constraint::Length(20),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
            ];
            let history_table = Table::new(history_rows, history_widths)
                .header(Row::new(vec!["Task ID", "Agent", "Status", "Attempts"]))
                .block(
                    Block::default()
                        .title("Task History (last 100)")
                        .borders(Borders::ALL),
                );
            frame.render_widget(history_table, queue_middle_chunks[1]);

            let bottom_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(vertical_chunks[2]);

            let log_entries = queue.recent_logs(8).unwrap_or_default();
            let log_rows: Vec<Row> = log_entries
                .iter()
                .map(|entry| {
                    Row::new(vec![
                        entry.task_id.clone(),
                        entry.level.clone(),
                        truncate(&entry.message, 24),
                        truncate(
                            &entry
                                .details
                                .as_ref()
                                .map(|value| value.to_string())
                                .unwrap_or_default(),
                            32,
                        ),
                        format!("{}", entry.created_at),
                    ])
                })
                .collect();

            let log_widths = [
                Constraint::Length(18),
                Constraint::Length(10),
                Constraint::Length(24),
                Constraint::Length(32),
                Constraint::Length(10),
            ];
            let log_table = Table::new(log_rows, log_widths)
                .header(Row::new(vec![
                    "Task ID",
                    "Level",
                    "Message",
                    "Details",
                    "Created",
                ]))
                .block(
                    Block::default()
                        .title("Worker Logs")
                        .borders(Borders::ALL),
                );
            frame.render_widget(log_table, bottom_chunks[0]);

            let file_events = queue.recent_file_events(8).unwrap_or_default();
            let file_event_rows: Vec<Row> = file_events
                .iter()
                .map(|event| {
                    Row::new(vec![
                        event.path.clone(),
                        event.event.clone(),
                        event.source.clone(),
                        truncate(
                            &event
                                .details
                                .as_ref()
                                .map(|value| value.to_string())
                                .unwrap_or_default(),
                            30,
                        ),
                        format!("{}", event.created_at),
                    ])
                })
                .collect();

            let file_event_widths = [
                Constraint::Length(22),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(30),
                Constraint::Length(10),
            ];
            let file_event_table = Table::new(file_event_rows, file_event_widths)
                .header(Row::new(vec![
                    "Path",
                    "Event",
                    "Source",
                    "Details",
                    "Logged",
                ]))
                .block(
                    Block::default()
                        .title("File Modifications")
                        .borders(Borders::ALL),
                );

            let files_stack = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(9), Constraint::Min(5)])
                .split(bottom_chunks[1]);
            frame.render_widget(file_event_table, files_stack[0]);

            let modified_text = {
                let files = config.modified_files.lock().unwrap();
                if files.is_empty() {
                    "No recent modifications".to_string()
                } else {
                    files
                        .iter()
                        .take(5)
                        .map(|file| file.clone())
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            };
            let modified_para = Paragraph::new(modified_text)
                .block(
                    Block::default()
                        .title("Recent Local Modifications")
                        .borders(Borders::ALL),
                );
            frame.render_widget(modified_para, files_stack[1]);
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

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        value.to_string()
    } else {
        let mut truncated = value[..max].to_string();
        truncated.push('…');
        truncated
    }
}

#[cfg(not(feature = "tui"))]
pub fn run_dashboard(_queue: &crate::queue::SqliteQueue) -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "tui feature not enabled; rebuild with --features tui"
    ))
}

#[cfg(not(feature = "tui"))]
pub struct TuiConfig {
    pub db_path: String,
    pub worker_count: usize,
    pub agent_count: usize,
}

#[cfg(not(feature = "tui"))]
pub fn run_dashboard_with_config(
    _queue: &crate::queue::SqliteQueue,
    _config: TuiConfig,
) -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "tui feature not enabled; rebuild with --features tui"
    ))
}
