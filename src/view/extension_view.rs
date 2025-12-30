use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::util::format_size;
use crate::viewmodel::ExtensionViewModel;

use super::ui_fmt;

pub fn render(frame: &mut Frame, vm: &ExtensionViewModel, area: Rect) {
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

fn render_header(frame: &mut Frame, vm: &ExtensionViewModel, area: Rect) {
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("FILE TYPE BREAKDOWN", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::raw(format!("Total: {} cumulative, {} current, {} files",
                format_size(vm.total_cumulative()),
                format_size(vm.total_current()),
                vm.total_files())),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title("Size by File Extension"));
    frame.render_widget(header, area);
}

fn render_list(frame: &mut Frame, vm: &ExtensionViewModel, area: Rect) {
    let total_cumulative = vm.total_cumulative();
    let stats = vm.stats();

    let items: Vec<ListItem> = stats
        .iter()
        .map(|stat| {
            let percent = ui_fmt::percent(stat.cumulative_size, total_cumulative);
            let bloat = ui_fmt::bloat_ratio(stat.cumulative_size, stat.current_size);
            let bloat_str = ui_fmt::bloat_str(stat.cumulative_size, stat.current_size);
            let bar = ui_fmt::bar(percent, 20);
            let bloat_color = ui_fmt::bloat_color(bloat);

            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>12}", &stat.extension), Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                Span::styled(format!("{:>10}", format_size(stat.cumulative_size)), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(format!("[{}]", bar), Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled(format!("{:>5}", bloat_str), Style::default().fg(bloat_color)),
                Span::raw("  "),
                Span::styled(format!("{:>8}", format_size(stat.current_size)), Style::default().fg(Color::White)),
                Span::raw("  "),
                Span::styled(format!("{:>6} files", stat.file_count), Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(vm.selected_index()));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!("Extensions ({} types)", stats.len())))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(Color::Yellow)), Span::raw(" nav  "),
        Span::styled("/", Style::default().fg(Color::Yellow)), Span::raw(" search  "),
        Span::styled("t/Esc", Style::default().fg(Color::Yellow)), Span::raw(" tree  "),
        Span::styled("q", Style::default().fg(Color::Yellow)), Span::raw(" quit  |  "),
        Span::styled("CUM", Style::default().fg(Color::Cyan)), Span::raw(" "),
        Span::styled("BLOAT", Style::default().fg(Color::Green)), Span::raw(" "),
        Span::styled("CUR", Style::default().fg(Color::White)), Span::raw(" "),
        Span::styled("FILES", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, area);
}
