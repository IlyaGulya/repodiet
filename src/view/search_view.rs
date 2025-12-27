use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::util::format_size;
use crate::viewmodel::SearchViewModel;

pub fn render(frame: &mut Frame, vm: &SearchViewModel, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Search input
            Constraint::Min(0),     // Results
            Constraint::Length(3),  // Footer
        ])
        .split(area);

    render_header(frame, vm, chunks[0]);
    render_results(frame, vm, chunks[1]);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, vm: &SearchViewModel, area: Rect) {
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("SEARCH", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw(" > "),
            Span::styled(vm.query(), Style::default().fg(Color::Yellow)),
            Span::styled("█", Style::default().fg(Color::White)),  // Cursor
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title("Search Files (type to filter)"));
    frame.render_widget(header, area);
}

fn render_results(frame: &mut Frame, vm: &SearchViewModel, area: Rect) {
    let total_cumulative = vm.total_cumulative();
    let results = vm.results();

    let items: Vec<ListItem> = results
        .iter()
        .map(|result| {
            let percent = if total_cumulative > 0 {
                result.cumulative_size as f64 / total_cumulative as f64 * 100.0
            } else {
                0.0
            };

            let bloat = if result.current_size > 0 {
                result.cumulative_size as f64 / result.current_size as f64
            } else {
                f64::INFINITY
            };

            let bloat_str = if bloat.is_infinite() { "DEL".to_string() } else { format!("{:.1}x", bloat) };

            let bar_width = 15;
            let filled = ((percent / 100.0) * bar_width as f64) as usize;
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

            let bloat_color = if bloat > 50.0 { Color::Red } else if bloat > 20.0 { Color::Yellow } else { Color::Green };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>10}", format_size(result.cumulative_size)), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(format!("[{}]", bar), Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled(format!("{:>5}", bloat_str), Style::default().fg(bloat_color)),
                Span::raw("  "),
                Span::raw(&result.path),
            ]))
        })
        .collect();

    let result_count = results.len();
    let mut list_state = ListState::default();
    list_state.select(Some(vm.selected_index()));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Results ({}{} matches)",
            result_count,
            if result_count >= 100 { "+" } else { "" }
        )))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(Color::Yellow)), Span::raw(" nav  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)), Span::raw(" go to  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)), Span::raw(" cancel  "),
        Span::styled("Backspace", Style::default().fg(Color::Yellow)), Span::raw(" delete"),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, area);
}
