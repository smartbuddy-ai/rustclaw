use ratatui::style::Color;
use super::theme::MenuColors;

/// Status indicator for menu items
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    Active,   // Green dot
    Idle,     // Amber dot
    Error,    // Red dot
    Disabled, // Gray dot
}

impl Status {
    pub fn color(&self) -> Color {
        match self {
            Status::Active => MenuColors::SUCCESS,
            Status::Idle => MenuColors::WARNING,
            Status::Error => MenuColors::ERROR,
            Status::Disabled => MenuColors::FG_DIM,
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Status::Active => "●",
            Status::Idle => "●",
            Status::Error => "●",
            Status::Disabled => "○",
        }
    }
}

/// A collapsible section in the left panel
#[derive(Debug, Clone)]
pub struct MenuSection {
    pub title: String,
    pub color: Color,
    pub icon: String,
    pub expanded: bool,
    pub items: Vec<MenuItem>,
}

/// An item inside a section
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub status: Status,
    pub detail: Option<String>,
    pub expanded: bool,
    pub content: Option<String>, // Expandable content (like parameters/result)
}

impl MenuItem {
    pub fn new(label: &str, status: Status) -> Self {
        Self {
            label: label.to_string(),
            status,
            detail: None,
            expanded: false,
            content: None,
        }
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_content(mut self, content: &str) -> Self {
        self.content = Some(content.to_string());
        self
    }
}

/// The full left panel state
#[derive(Debug)]
pub struct LeftPanel {
    pub sections: Vec<MenuSection>,
    pub selected_section: usize,
    pub selected_item: Option<usize>,
    pub focused: bool,
}

impl LeftPanel {
    pub fn new() -> Self {
        Self {
            sections: vec![
                MenuSection {
                    title: "Channels".into(),
                    color: MenuColors::CHANNELS,
                    icon: "📡".into(),
                    expanded: true,
                    items: vec![
                        MenuItem::new("Telegram", Status::Active)
                            .with_detail("connected"),
                        MenuItem::new("WhatsApp", Status::Disabled)
                            .with_detail("not configured"),
                        MenuItem::new("Slack", Status::Disabled)
                            .with_detail("not configured"),
                    ],
                },
                MenuSection {
                    title: "Agents".into(),
                    color: MenuColors::AGENTS,
                    icon: "🤖".into(),
                    expanded: true,
                    items: vec![
                        MenuItem::new("main", Status::Active)
                            .with_detail("claude-sonnet-4-5"),
                    ],
                },
                MenuSection {
                    title: "Cron Jobs".into(),
                    color: MenuColors::CRON,
                    icon: "⏱".into(),
                    expanded: false,
                    items: vec![
                        MenuItem::new("heartbeat", Status::Idle)
                            .with_detail("every 30m"),
                    ],
                },
                MenuSection {
                    title: "Nodes".into(),
                    color: MenuColors::NODES,
                    icon: "🖥".into(),
                    expanded: false,
                    items: vec![
                        MenuItem::new("localhost", Status::Active)
                            .with_detail("this machine"),
                    ],
                },
                MenuSection {
                    title: "Workspace".into(),
                    color: MenuColors::WORKSPACE,
                    icon: "📁".into(),
                    expanded: false,
                    items: vec![
                        MenuItem::new("SOUL.md", Status::Active),
                        MenuItem::new("MEMORY.md", Status::Active),
                        MenuItem::new("AGENTS.md", Status::Active),
                        MenuItem::new("TOOLS.md", Status::Active),
                    ],
                },
                MenuSection {
                    title: "Settings".into(),
                    color: MenuColors::SETTINGS,
                    icon: "⚙".into(),
                    expanded: false,
                    items: vec![
                        MenuItem::new("Config", Status::Active)
                            .with_detail("config.toml"),
                        MenuItem::new("Secrets", Status::Active)
                            .with_detail(".env"),
                    ],
                },
            ],
            selected_section: 0,
            selected_item: None,
            focused: true,
        }
    }

    /// Toggle expand/collapse for the selected section
    pub fn toggle_section(&mut self) {
        if self.selected_item.is_none() {
            if let Some(sec) = self.sections.get_mut(self.selected_section) {
                sec.expanded = !sec.expanded;
            }
        } else if let Some(item_idx) = self.selected_item {
            if let Some(sec) = self.sections.get_mut(self.selected_section) {
                if let Some(item) = sec.items.get_mut(item_idx) {
                    if item.content.is_some() {
                        item.expanded = !item.expanded;
                    }
                }
            }
        }
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if let Some(item_idx) = self.selected_item {
            if item_idx > 0 {
                self.selected_item = Some(item_idx - 1);
            } else {
                self.selected_item = None; // Back to section header
            }
        } else if self.selected_section > 0 {
            self.selected_section -= 1;
            let sec = &self.sections[self.selected_section];
            if sec.expanded && !sec.items.is_empty() {
                self.selected_item = Some(sec.items.len() - 1);
            }
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        let sec = &self.sections[self.selected_section];
        if let Some(item_idx) = self.selected_item {
            if item_idx + 1 < sec.items.len() {
                self.selected_item = Some(item_idx + 1);
            } else if self.selected_section + 1 < self.sections.len() {
                self.selected_section += 1;
                self.selected_item = None;
            }
        } else if sec.expanded && !sec.items.is_empty() {
            self.selected_item = Some(0);
        } else if self.selected_section + 1 < self.sections.len() {
            self.selected_section += 1;
            self.selected_item = None;
        }
    }
}
