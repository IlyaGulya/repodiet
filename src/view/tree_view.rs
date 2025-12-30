use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::util::format_size;
use crate::viewmodel::TreeViewModel;

use super::ui_fmt;

pub fn render(frame: &mut Frame, vm: &TreeViewModel, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // List
            Constraint::Length(3),  // Footer
        ])
        .split(area);

    render_header(frame, vm, chunks[0]);
    render_list(frame, vm, chunks[1]);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, vm: &TreeViewModel, area: Rect) {
    let current_path = vm.current_path();
    let current = vm.current_node();

    let header_text = if vm.is_deleted_only() {
        let deleted_size = current.deleted_cumulative_size();
        format!("Deleted: {} (reclaimable) [DELETED ONLY]", format_size(deleted_size))
    } else {
        let bloat = if current.current_size > 0 {
            current.cumulative_size as f64 / current.current_size as f64
        } else {
            f64::INFINITY
        };
        format!("Cumulative: {} | Current: {} | Bloat: {:.1}x",
            format_size(current.cumulative_size),
            format_size(current.current_size),
            bloat)
    };

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::raw("Path: "),
            Span::styled(&current_path, Style::default().fg(Color::Yellow)),
            Span::raw(" | "),
            Span::raw(header_text),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title("repodiet"));
    frame.render_widget(header, area);
}

fn render_list(frame: &mut Frame, vm: &TreeViewModel, area: Rect) {
    let show_deleted = vm.is_deleted_only();
    let total_for_percent = vm.total_for_percent();
    let children = vm.visible_children();
    let num_children = children.len();

    let items: Vec<ListItem> = children
        .iter()
        .map(|node| {
            let percent = ui_fmt::percent(node.display_size, total_for_percent);

            let bloat_str = if show_deleted {
                "DEL".to_string()
            } else {
                ui_fmt::bloat_str(node.display_size, node.current_size)
            };

            let bar = ui_fmt::bar(percent, 20);

            let prefix = if node.has_children { "▸ " } else { "  " };
            let size_color = if show_deleted { Color::Magenta } else { Color::Cyan };

            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::styled(format!("{:>10}", format_size(node.display_size)), Style::default().fg(size_color)),
                Span::raw(" "),
                Span::styled(format!("[{}]", bar), Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled(format!("{:>5}", bloat_str), Style::default().fg(Color::Red)),
                Span::raw(" "),
                Span::raw(&node.name),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(vm.selected_index()));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!("Contents ({} items)", num_children)))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(Color::Yellow)), Span::raw(" nav  "),
        Span::styled("Enter/→", Style::default().fg(Color::Yellow)), Span::raw(" enter  "),
        Span::styled("←", Style::default().fg(Color::Yellow)), Span::raw(" back  "),
        Span::styled("/", Style::default().fg(Color::Yellow)), Span::raw(" search  "),
        Span::styled("d", Style::default().fg(Color::Yellow)), Span::raw(" deleted  "),
        Span::styled("t", Style::default().fg(Color::Yellow)), Span::raw(" types  "),
        Span::styled("L", Style::default().fg(Color::Yellow)), Span::raw(" blobs  "),
        Span::styled("q", Style::default().fg(Color::Yellow)), Span::raw(" quit"),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, area);
}
