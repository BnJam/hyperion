#[cfg(feature = "tui")]
use std::{
    collections::VecDeque,
    fs,
    io::{self, IsTerminal},
    path::Path,
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
use crate::models::{ApprovalRecord, QueueStatus};
#[cfg(feature = "tui")]
use crate::queue::SqliteQueue;
#[cfg(feature = "tui")]
use serde::Deserialize;
#[cfg(feature = "tui")]
use serde_json::Value;

#[cfg(feature = "tui")]
const STATUS_FILTERS: [Option<QueueStatus>; 5] = [
    None,
    Some(QueueStatus::Pending),
    Some(QueueStatus::InProgress),
    Some(QueueStatus::Applied),
    Some(QueueStatus::Failed),
];

#[cfg(feature = "tui")]
const REFRESH_INTERVALS: [u64; 4] = [200, 500, 1000, 2000];

#[cfg(feature = "tui")]
pub struct TuiConfig {
    pub db_path: String,
    pub worker_count: usize,
    pub agent_count: usize,
    pub modified_files: Arc<Mutex<VecDeque<String>>>,
}

#[cfg(feature = "tui")]
struct TuiState {
    status_index: usize,
    agent_filter: Option<String>,
    refresh_index: usize,
    selected_index: usize,
    show_detail: bool,
    show_events: bool,
}

#[cfg(feature = "tui")]
impl Default for TuiState {
    fn default() -> Self {
        Self {
            status_index: 0,
            agent_filter: None,
            refresh_index: 1,
            selected_index: 0,
            show_detail: true,
            show_events: true,
        }
    }
}

#[cfg(feature = "tui")]
pub fn run_dashboard(queue: &SqliteQueue) -> anyhow::Result<()> {
    run_dashboard_with_config(
        queue,
        TuiConfig {
            db_path: "hyperion.db".to_string(),
            worker_count: 0,
            agent_count: 0,
            modified_files: Arc::new(Mutex::new(VecDeque::new())),
        },
    )
}

#[cfg(feature = "tui")]
pub fn run_dashboard_with_config(queue: &SqliteQueue, config: TuiConfig) -> anyhow::Result<()> {
    if !io::stdout().is_terminal() {
        eprintln!("TUI requires a terminal; skipping dashboard.");
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut state = TuiState::default();

    loop {
        let pending_records = queue.list(QueueStatus::Pending).unwrap_or_default();
        let in_progress_records = queue.list(QueueStatus::InProgress).unwrap_or_default();
        let applied_count = queue
            .list(QueueStatus::Applied)
            .map(|records| records.len())
            .unwrap_or(0usize);
        let failed_count = queue
            .list(QueueStatus::Failed)
            .map(|records| records.len())
            .unwrap_or(0usize);
        let dead_letters = queue.dead_letter_count().unwrap_or(0);
        let history_records = queue.recent_records(100).unwrap_or_default();
        let metrics = queue.queue_metrics(Some(60)).unwrap_or_default();
        let format_metric = |value: Option<f64>, suffix: &str| {
            value
                .map(|v| format!("{:.1}{}", v, suffix))
                .unwrap_or_else(|| "n/a".to_string())
        };
        let mut queue_records = history_records.clone();
        if let Some(filter) = STATUS_FILTERS[state.status_index] {
            queue_records.retain(|record| record.status == filter);
        }
        if let Some(agent) = state.agent_filter.clone() {
            queue_records.retain(|record| record.payload.agent == agent);
        }
        if queue_records.is_empty() {
            state.selected_index = 0;
        } else if state.selected_index >= queue_records.len() {
            state.selected_index = queue_records.len() - 1;
        }

        let log_entries = queue.recent_logs(8).unwrap_or_default();
        let file_events = queue.recent_file_events(8).unwrap_or_default();
        let agent_names = {
            let mut names: Vec<String> = history_records
                .iter()
                .map(|record| record.payload.agent.clone())
                .collect();
            names.sort();
            names.dedup();
            names
        };

        terminal.draw(|frame| {
            let area = frame.size();
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(9),
                Constraint::Length(12),
                Constraint::Min(12),
            ])
            .split(area);

        let header_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Length(3)])
            .split(vertical_chunks[0]);

            let status_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                ])
                .split(header_rows[0]);

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

            let status_label = match STATUS_FILTERS[state.status_index] {
                Some(status) => status.as_str(),
                None => "all",
            };
            let agent_label = state
                .agent_filter
                .as_deref()
                .unwrap_or("all agents")
                .to_string();
            let guidance_text = format!(
                "Controls\nq: Quit\ns: Cycle status filter ({status_label})\na: Cycle agent ({agent_label})\nr: Refresh {refresh}ms\nd: Toggle detail pane ({})\ne: Toggle events ({})\narrow/↓: move selection\nhyperion request <file>: enqueue task request\nhyperion session init --resume=<token> [--model=<name>] [--allow-all-tools=<bool>]\nhyperion session list: show stored Copilot sessions\nhyperion queue-metrics --format json --since {window}: export throughput/latency/lease stats\n`hyperion run` / `hyperion worker` print `[progress]` lines with the same metrics before the TUI opens\n",
                if state.show_detail { "on" } else { "off" },
                if state.show_events { "on" } else { "off" },
                refresh = REFRESH_INTERVALS[state.refresh_index],
                window = metrics.window_seconds,
            );
            let guidance = Paragraph::new(guidance_text)
                .block(Block::default().title("Guidance").borders(Borders::ALL));
            frame.render_widget(guidance, status_chunks[2]);

            let agent_requests = format_metric(metrics.agent_requests_per_second, "/s");
            let agent_complexity = format_metric(metrics.agent_average_complexity, "");
            let guard_success_rate = metrics
                .agent_guard_success_rate
                .map(|value| format!("{:.1}%", value * 100.0))
                .unwrap_or_else(|| "n/a".to_string());
            let approval_latency =
                format_metric(metrics.agent_guard_approval_latency_ms, "ms");
            let metrics_text = format!(
                "Window: {}s\nThroughput: {}\nAvg dequeue latency: {}\nAvg apply duration: {}\nAvg poll interval: {}\nLease contention events: {}\n\nAgent telemetry\nRequests/s: {}\nAvg complexity: {}\nGuard success rate: {}\nApproval latency: {}\nQueue metrics panel + `[progress]` logs share these numbers; run `hyperion queue-metrics --format json --since {}` for structured output.",
                metrics.window_seconds,
                format_metric(metrics.throughput_per_minute, "/min"),
                format_metric(metrics.avg_dequeue_latency_ms, "ms"),
                format_metric(metrics.avg_apply_duration_ms, "ms"),
                format_metric(metrics.avg_poll_interval_ms, "ms"),
                metrics.lease_contention_events,
                agent_requests,
                agent_complexity,
                guard_success_rate,
                approval_latency,
                metrics.window_seconds,
            );
            let metrics_block = Paragraph::new(metrics_text)
                .block(Block::default().title("Metrics").borders(Borders::ALL));
            frame.render_widget(metrics_block, header_rows[1]);

            let queue_middle_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(vertical_chunks[1]);

            let queue_rows: Vec<Row> = queue_records
                .iter()
                .enumerate()
                .take(10)
                .map(|(idx, record)| {
                    Row::new(vec![
                        record.id.to_string(),
                        record.payload.task_id.clone(),
                        record.payload.agent.clone(),
                        record.status.as_str().to_string(),
                        record.attempts.to_string(),
                        if idx == state.selected_index {
                            "➜".to_string()
                        } else {
                            "".to_string()
                        },
                    ])
                })
                .collect();

            let queue_widths = [
                Constraint::Length(6),
                Constraint::Length(20),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(3),
            ];
            let queue_table = Table::new(queue_rows, queue_widths)
                .header(Row::new(vec![
                    "ID",
                    "Task ID",
                    "Agent",
                    "Status",
                    "Attempts",
                    "Sel",
                ]))
                .block(
                    Block::default()
                        .title("Recent Queue Entries")
                        .borders(Borders::ALL),
                );
            frame.render_widget(queue_table, queue_middle_chunks[0]);

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
                                .map(|value: &Value| value.to_string())
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
            let log_block = Block::default()
                .title("Worker Logs")
                .borders(Borders::ALL);
            if state.show_events {
                let log_table = Table::new(log_rows, log_widths)
                    .header(Row::new(vec![
                        "Task ID",
                        "Level",
                        "Message",
                        "Details",
                        "Created",
                    ]))
                    .block(log_block);
                frame.render_widget(log_table, bottom_chunks[0]);
            } else {
                let paused = Paragraph::new("Worker event stream paused. Press 'e' to resume.")
                    .block(
                        log_block
                            .clone()
                            .title("Worker Logs (paused)")
                            .borders(Borders::ALL),
                    );
                frame.render_widget(paused, bottom_chunks[0]);
            }

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
                                .map(|value: &Value| value.to_string())
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

            let cast_context = load_cast_builder_context();
            let cast_text = format_cast_builder_text(cast_context.as_ref());

            let details_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(9),
                    Constraint::Length(7),
                    Constraint::Length(6),
                    Constraint::Min(5),
                ])
                .split(bottom_chunks[1]);
            frame.render_widget(file_event_table, details_chunks[0]);

            let cast_para = Paragraph::new(cast_text)
                .block(
                    Block::default()
                        .title("Cast Builder Status")
                        .borders(Borders::ALL),
                );
            frame.render_widget(cast_para, details_chunks[1]);

            let detail_text = if state.show_detail {
                if let Some(record) = queue_records.get(state.selected_index) {
                    format!(
                        "Task: {}\nAgent: {}\nStatus: {}\nAttempts: {}\nLease until: {:?}\nLease owner: {}\nLast error: {}",
                        record.payload.task_id,
                        record.payload.agent,
                        record.status.as_str(),
                        record.attempts,
                        record.leased_until,
                        record
                            .lease_owner
                            .clone()
                            .unwrap_or_else(|| "<none>".to_string()),
                        record
                            .last_error
                            .clone()
                            .unwrap_or_else(|| "<none>".to_string())
                    )
                } else {
                    "No queue records match the current filter.".to_string()
                }
            } else {
                "Detail pane hidden. Press 'd' to show.".to_string()
            };
            let detail_para = Paragraph::new(detail_text)
                .block(
                    Block::default()
                        .title("Selection Detail")
                        .borders(Borders::ALL),
                );
            frame.render_widget(detail_para, details_chunks[2]);

            let modified_text = {
                let files = config.modified_files.lock().unwrap();
                if files.is_empty() {
                    "No recent modifications".to_string()
                } else {
                    files
                .iter()
                .take(5)
                .cloned()
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
            frame.render_widget(modified_para, details_chunks[3]);
        })?;

        let refresh_duration = Duration::from_millis(REFRESH_INTERVALS[state.refresh_index]);
        if event::poll(refresh_duration)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('s') => {
                        state.status_index = (state.status_index + 1) % STATUS_FILTERS.len();
                        state.selected_index = 0;
                    }
                    KeyCode::Char('a') => {
                        state.agent_filter =
                            cycle_agent_filter(&agent_names, state.agent_filter.as_deref());
                        state.selected_index = 0;
                    }
                    KeyCode::Char('r') => {
                        state.refresh_index = (state.refresh_index + 1) % REFRESH_INTERVALS.len();
                    }
                    KeyCode::Char('d') => {
                        state.show_detail = !state.show_detail;
                    }
                    KeyCode::Char('e') => {
                        state.show_events = !state.show_events;
                    }
                    KeyCode::Up => {
                        if state.selected_index > 0 {
                            state.selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if !queue_records.is_empty() {
                            state.selected_index =
                                (state.selected_index + 1).min(queue_records.len() - 1);
                        }
                    }
                    _ => {}
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

#[cfg(feature = "tui")]
#[derive(Debug, Clone, Deserialize)]
struct CastBuilderContext {
    request_id: String,
    summary: String,
    intent: String,
    complexity: u8,
    telemetry_anchors: Vec<String>,
    approvals: Vec<ApprovalRecord>,
    exported_at: Option<u64>,
}

#[cfg(feature = "tui")]
fn load_cast_builder_context() -> Option<CastBuilderContext> {
    let path = Path::new("execution/next_task_context.json");
    let text = fs::read_to_string(path).ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    data.get("cast_builder")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

#[cfg(feature = "tui")]
fn format_cast_builder_text(ctx: Option<&CastBuilderContext>) -> String {
    if let Some(context) = ctx {
        let anchors = if context.telemetry_anchors.is_empty() {
            "none".to_string()
        } else {
            context.telemetry_anchors.join(", ")
        };
        let approvals = if context.approvals.is_empty() {
            "none".to_string()
        } else {
            context
                .approvals
                .iter()
                .map(|approval| approval.approver.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let exported = context
            .exported_at
            .map(|value| format!("{value}s since epoch"))
            .unwrap_or_else(|| "n/a".to_string());
        format!(
            "Request: {}\nSummary: {}\nIntent: {}\nComplexity: {}/10\nAnchors: {}\nApprovals: {}\nExported: {}",
            context.request_id,
            context.summary,
            context.intent,
            context.complexity,
            anchors,
            approvals,
            exported,
        )
    } else {
        "Cast Builder: no exports yet.".to_string()
    }
}

#[cfg(feature = "tui")]
fn cycle_agent_filter(names: &[String], current: Option<&str>) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    if let Some(current) = current {
        if let Some(pos) = names.iter().position(|name| name == current) {
            if pos + 1 < names.len() {
                return Some(names[pos + 1].clone());
            }
            return None;
        }
    }
    Some(names[0].clone())
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
