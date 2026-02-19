use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Widget, Paragraph, Wrap},
};

use super::panel::LeftPanel;
use super::theme::MenuColors;

/// Renders the left panel with collapsible sections and colored items.
pub struct LeftPanelWidget<'a> {
    pub panel: &'a LeftPanel,
}

impl<'a> Widget for LeftPanelWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Background
        let block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(MenuColors::BORDER))
            .style(Style::default().bg(MenuColors::BG_PANEL))
            .padding(Padding::new(1, 1, 1, 0));

        let inner = block.inner(area);
        block.render(area, buf);

        // Title
        let title_line = Line::from(vec![
            Span::styled("  rustclaw ", Style::default()
                .fg(MenuColors::ACCENT)
                .add_modifier(Modifier::BOLD)),
            Span::styled("v0.1.0", Style::default().fg(MenuColors::FG_DIM)),
        ]);

        let mut y = inner.y;

        // Render title
        if y < inner.y + inner.height {
            buf.set_line(inner.x, y, &title_line, inner.width);
            y += 2; // title + blank line
        }

        // Render sections
        for (sec_idx, section) in self.panel.sections.iter().enumerate() {
            if y >= inner.y + inner.height {
                break;
            }

            let is_selected_section = sec_idx == self.panel.selected_section;
            let is_section_header_selected = is_selected_section && self.panel.selected_item.is_none();

            // Section header
            let chevron = if section.expanded { "▾" } else { "▸" };
            let bg = if is_section_header_selected && self.panel.focused {
                MenuColors::BG_SELECTED
            } else {
                MenuColors::BG_PANEL
            };

            let header_line = Line::from(vec![
                Span::styled(
                    format!(" {} ", chevron),
                    Style::default().fg(section.color).bg(bg),
                ),
                Span::styled(
                    format!("{} ", section.icon),
                    Style::default().fg(section.color).bg(bg),
                ),
                Span::styled(
                    section.title.clone(),
                    Style::default()
                        .fg(if is_section_header_selected { MenuColors::FG_BRIGHT } else { MenuColors::FG_TEXT })
                        .bg(bg)
                        .add_modifier(if is_section_header_selected { Modifier::BOLD } else { Modifier::empty() }),
                ),
                // Fill rest of line with bg
                Span::styled(
                    " ".repeat(inner.width.saturating_sub(section.title.len() as u16 + 6) as usize),
                    Style::default().bg(bg),
                ),
            ]);

            buf.set_line(inner.x, y, &header_line, inner.width);
            y += 1;

            // Items (if expanded)
            if section.expanded {
                for (item_idx, item) in section.items.iter().enumerate() {
                    if y >= inner.y + inner.height {
                        break;
                    }

                    let is_item_selected = is_selected_section
                        && self.panel.selected_item == Some(item_idx);

                    let bg = if is_item_selected && self.panel.focused {
                        MenuColors::BG_SELECTED
                    } else {
                        MenuColors::BG_PANEL
                    };

                    // Status dot + label
                    let status_dot = Span::styled(
                        format!("   {} ", item.status.symbol()),
                        Style::default().fg(item.status.color()).bg(bg),
                    );

                    let label = Span::styled(
                        item.label.clone(),
                        Style::default()
                            .fg(if is_item_selected { MenuColors::FG_BRIGHT } else { MenuColors::FG_TEXT })
                            .bg(bg)
                            .add_modifier(if is_item_selected { Modifier::BOLD } else { Modifier::empty() }),
                    );

                    let mut spans = vec![status_dot, label];

                    // Detail text (dimmed, right-aligned feel)
                    if let Some(ref detail) = item.detail {
                        spans.push(Span::styled(
                            format!(" {}", detail),
                            Style::default().fg(MenuColors::FG_DIM).bg(bg),
                        ));
                    }

                    // Item chevron if has content
                    if item.content.is_some() {
                        let chev = if item.expanded { " ▴" } else { " ▾" };
                        spans.push(Span::styled(
                            chev.to_string(),
                            Style::default().fg(MenuColors::FG_DIM).bg(bg),
                        ));
                    }

                    // Fill rest
                    let used: usize = spans.iter().map(|s| s.content.len()).sum();
                    let remaining = inner.width.saturating_sub(used as u16) as usize;
                    spans.push(Span::styled(
                        " ".repeat(remaining),
                        Style::default().bg(bg),
                    ));

                    let item_line = Line::from(spans);
                    buf.set_line(inner.x, y, &item_line, inner.width);
                    y += 1;

                    // Expanded content
                    if item.expanded {
                        if let Some(ref content) = item.content {
                            for line in content.lines() {
                                if y >= inner.y + inner.height {
                                    break;
                                }
                                let content_line = Line::from(vec![
                                    Span::styled(
                                        format!("     {}", line),
                                        Style::default()
                                            .fg(MenuColors::FG_DIM)
                                            .bg(MenuColors::BG_PANEL),
                                    ),
                                ]);
                                buf.set_line(inner.x, y, &content_line, inner.width);
                                y += 1;
                            }
                        }
                    }
                }
            }

            // Spacing between sections
            y += 1;
        }
    }
}

/// Renders a simple chat area (right panel placeholder).
pub struct ChatWidget<'a> {
    pub messages: &'a [(String, String)], // (role, content)
    pub input: &'a str,
}

impl<'a> Widget for ChatWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::NONE)
            .style(Style::default().bg(MenuColors::BG_PANEL))
            .padding(Padding::new(1, 1, 1, 1));

        let inner = block.inner(area);
        block.render(area, buf);

        let mut y = inner.y;

        // Messages
        for (role, content) in self.messages {
            if y >= inner.y + inner.height - 3 {
                break;
            }

            let role_color = if role == "user" {
                MenuColors::ACCENT
            } else {
                MenuColors::SUCCESS
            };

            let role_line = Line::from(Span::styled(
                format!(" {} ", role),
                Style::default().fg(role_color).add_modifier(Modifier::BOLD),
            ));
            buf.set_line(inner.x, y, &role_line, inner.width);
            y += 1;

            for line in content.lines() {
                if y >= inner.y + inner.height - 3 {
                    break;
                }
                let msg_line = Line::from(Span::styled(
                    format!(" {}", line),
                    Style::default().fg(MenuColors::FG_TEXT),
                ));
                buf.set_line(inner.x, y, &msg_line, inner.width);
                y += 1;
            }
            y += 1;
        }

        // Input bar at bottom
        let input_y = inner.y + inner.height - 1;
        if input_y > inner.y {
            // Separator
            let sep_y = input_y - 1;
            let sep = Line::from(Span::styled(
                "─".repeat(inner.width as usize),
                Style::default().fg(MenuColors::BORDER),
            ));
            buf.set_line(inner.x, sep_y, &sep, inner.width);

            // Input
            let prompt = Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(MenuColors::ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(
                    if self.input.is_empty() { "Type a message..." } else { self.input },
                    Style::default().fg(if self.input.is_empty() { MenuColors::FG_DIM } else { MenuColors::FG_TEXT }),
                ),
            ]);
            buf.set_line(inner.x, input_y, &prompt, inner.width);
        }
    }
}
