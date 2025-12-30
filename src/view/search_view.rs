use std::ops::Range;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::util::format_size;
use crate::viewmodel::SearchViewModel;

use super::ui_fmt;

/// Splits a path into spans, highlighting portions at the given byte ranges.
fn highlight_matches<'a>(path: &'a str, matches: &[Range<usize>]) -> Vec<Span<'a>> {
    if matches.is_empty() {
        return vec![Span::raw(path)];
    }

    let mut spans = Vec::new();
    let mut last_end = 0;

    for range in matches {
        if range.start > last_end {
            spans.push(Span::raw(&path[last_end..range.start]));
        }
        spans.push(Span::styled(
            &path[range.start..range.end],
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
        last_end = range.end;
    }

    if last_end < path.len() {
        spans.push(Span::raw(&path[last_end..]));
    }

    spans
}

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

    let items: Vec<ListItem> = vm
        .results()
        .map(|result| {
            let percent = ui_fmt::percent(result.cumulative_size, total_cumulative);
            let bloat = ui_fmt::bloat_ratio(result.cumulative_size, result.current_size);
            let bloat_str = ui_fmt::bloat_str(result.cumulative_size, result.current_size);
            let bar = ui_fmt::bar(percent, 15);
            let bloat_color = ui_fmt::bloat_color(bloat);

            let mut spans = vec![
                Span::styled(format!("{:>10}", format_size(result.cumulative_size)), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(format!("[{}]", bar), Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled(format!("{:>5}", bloat_str), Style::default().fg(bloat_color)),
                Span::raw("  "),
            ];
            spans.extend(highlight_matches(result.path, result.matches));

            ListItem::new(Line::from(spans))
        })
        .collect();
    let result_count = items.len();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_matches_single() {
        // "src/main.rs" - "main" is at bytes 4..8
        let spans = highlight_matches("src/main.rs", &[4..8]);
        assert_eq!(spans.len(), 3); // "src/", "main", ".rs"

        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "src/main.rs");
    }

    #[test]
    fn test_highlight_matches_multiple() {
        // "a/b/a.txt" - "a" is at bytes 0..1 and 4..5
        let spans = highlight_matches("a/b/a.txt", &[0..1, 4..5]);
        // "a", "/b/", "a", ".txt"
        assert_eq!(spans.len(), 4);

        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "a/b/a.txt");
    }

    #[test]
    fn test_highlight_matches_empty() {
        let spans = highlight_matches("src/main.rs", &[]);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "src/main.rs");
    }
}
