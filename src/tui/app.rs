use std::io;
use crossterm::{event::{self, Event, KeyCode, KeyEventKind}, execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use ratatui::{backend::CrosstermBackend, layout::{Constraint, Direction, Layout}, style::{Modifier, Style}, text::{Line, Span}, widgets::{Paragraph, Wrap}, Terminal};

use super::panel::LeftPanel;
use super::theme::MenuColors;
use super::widgets::{LeftPanelWidget, ChatWidget};

struct RuntimeState { channels_connected: usize, memory_count: usize, agent_status: String }

struct App {
    panel: LeftPanel,
    messages: Vec<(String, String)>,
    logs: Vec<String>,
    input: String,
    focus: Focus,
    should_quit: bool,
    state: RuntimeState,
    rt: tokio::runtime::Runtime,
}

#[derive(PartialEq)]
enum Focus { Panel, Chat }

impl App {
    fn new() -> Self {
        Self {
            panel: LeftPanel::new(),
            messages: vec![("system".into(), "Welcome to rustclaw live TUI".into())],
            logs: vec!["log stream ready".into()],
            input: String::new(),
            focus: Focus::Panel,
            should_quit: false,
            state: RuntimeState { channels_connected: 0, memory_count: 0, agent_status: "unknown".into() },
            rt: tokio::runtime::Runtime::new().unwrap(),
        }
    }

    fn refresh_state(&mut self) {
        let fut = async {
            let res = reqwest::get("http://127.0.0.1:8088/api/status").await.ok()?;
            let v: serde_json::Value = res.json().await.ok()?;
            Some((
                v.get("channels_connected").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
                v.get("memory_count").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
                v.get("status").and_then(|x| x.as_str()).unwrap_or("unknown").to_string(),
            ))
        };
        if let Some((c,m,s)) = self.rt.block_on(fut) {
            self.state.channels_connected = c;
            self.state.memory_count = m;
            self.state.agent_status = s;
        }
    }

    fn send_chat(&mut self, msg: String) {
        let fut = async move {
            let client = reqwest::Client::new();
            let res = client.post("http://127.0.0.1:8088/api/chat")
                .json(&serde_json::json!({"message": msg}))
                .send().await.ok()?;
            let v: serde_json::Value = res.json().await.ok()?;
            v.get("reply").and_then(|x| x.as_str()).map(|s| s.to_string())
        };
        if let Some(reply) = self.rt.block_on(fut) { self.messages.push(("assistant".into(), reply)); self.logs.push("chat roundtrip ok".into()); }
        else { self.messages.push(("assistant".into(), "gateway unavailable".into())); self.logs.push("chat failed".into()); }
    }

    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => self.focus = if self.focus == Focus::Panel { self.panel.focused = false; Focus::Chat } else { self.panel.focused=true; Focus::Panel },
            _ if self.focus == Focus::Panel => match key { KeyCode::Up|KeyCode::Char('k') => self.panel.move_up(), KeyCode::Down|KeyCode::Char('j') => self.panel.move_down(), KeyCode::Enter|KeyCode::Char(' ') => self.panel.toggle_section(), _=>{} },
            _ => match key {
                KeyCode::Enter => { if !self.input.is_empty() { let msg = std::mem::take(&mut self.input); self.messages.push(("user".into(), msg.clone())); self.send_chat(msg); } }
                KeyCode::Char(c) => self.input.push(c),
                KeyCode::Backspace => { self.input.pop(); },
                _ => {}
            }
        }
    }
}

pub fn run_tui() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    loop {
        app.refresh_state();
        terminal.draw(|frame| {
            let size = frame.area();
            let chunks = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(32), Constraint::Min(40)]).split(size);
            frame.render_widget(LeftPanelWidget { panel: &app.panel }, chunks[0]);

            let right = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(8), Constraint::Length(7)]).split(chunks[1]);
            frame.render_widget(ChatWidget { messages: &app.messages, input: &app.input }, right[0]);

            let log_text = app.logs.iter().rev().take(5).rev().cloned().collect::<Vec<_>>().join("\n");
            let logs = Paragraph::new(log_text).wrap(Wrap { trim: true }).style(Style::default().fg(MenuColors::FG_DIM));
            frame.render_widget(logs, right[1]);

            let status_chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(1)]).split(size);
            let status = Line::from(vec![
                Span::styled(" rustclaw ", Style::default().fg(MenuColors::BG_PANEL).bg(MenuColors::ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" {} | channels={} memory={} ", app.state.agent_status, app.state.channels_connected, app.state.memory_count), Style::default().fg(MenuColors::FG_BRIGHT).bg(MenuColors::BG_SELECTED)),
                Span::styled(" q quit | tab switch | enter send ", Style::default().fg(MenuColors::FG_DIM).bg(MenuColors::BG_SELECTED)),
            ]);
            frame.render_widget(status, status_chunks[1]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? { if key.kind == KeyEventKind::Press { app.handle_key(key.code); } }
        }
        if app.should_quit { break; }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
