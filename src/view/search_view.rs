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

/// Splits a path into spans, highlighting portions that match the query (case-insensitive).
fn highlight_matches<'a>(path: &'a str, query: &str) -> Vec<Span<'a>> {
    if query.is_empty() {
        return vec![Span::raw(path)];
    }

    let path_lower = path.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in path_lower.match_indices(&query_lower) {
        if start > last_end {
            spans.push(Span::raw(&path[last_end..start]));
        }
        spans.push(Span::styled(
            &path[start..start + query.len()],
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
        last_end = start + query.len();
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
    let query = vm.query();

    let items: Vec<ListItem> = vm
        .results()
        .map(|(path, cumulative_size, current_size)| {
            let percent = ui_fmt::percent(cumulative_size, total_cumulative);
            let bloat = ui_fmt::bloat_ratio(cumulative_size, current_size);
            let bloat_str = ui_fmt::bloat_str(cumulative_size, current_size);
            let bar = ui_fmt::bar(percent, 15);
            let bloat_color = ui_fmt::bloat_color(bloat);

            let mut spans = vec![
                Span::styled(format!("{:>10}", format_size(cumulative_size)), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(format!("[{}]", bar), Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled(format!("{:>5}", bloat_str), Style::default().fg(bloat_color)),
                Span::raw("  "),
            ];
            spans.extend(highlight_matches(path, query));

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
        let spans = highlight_matches("src/main.rs", "main");
        assert_eq!(spans.len(), 3); // "src/", "main", ".rs"

        // Check content
        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "src/main.rs");
    }

    #[test]
    fn test_highlight_matches_case_insensitive() {
        let spans = highlight_matches("README.md", "readme");
        assert_eq!(spans.len(), 2); // "README", ".md"

        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "README.md");
    }

    #[test]
    fn test_highlight_matches_multiple() {
        let spans = highlight_matches("a/b/a.txt", "a");
        // "a", "/b/", "a", ".txt"
        assert_eq!(spans.len(), 4);

        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "a/b/a.txt");
    }

    #[test]
    fn test_highlight_matches_empty_query() {
        let spans = highlight_matches("src/main.rs", "");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "src/main.rs");
    }

    #[test]
    fn test_highlight_matches_no_match() {
        let spans = highlight_matches("src/main.rs", "xyz");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "src/main.rs");
    }
}
