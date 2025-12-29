use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::util::{format_size, format_timestamp};
use crate::viewmodel::BlobsViewModel;

pub fn render(frame: &mut Frame, vm: &BlobsViewModel, area: Rect) {
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

fn render_header(frame: &mut Frame, vm: &BlobsViewModel, area: Rect) {
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("LARGE BLOB DETECTIVE", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::raw(format!("Top {} largest blobs: {} total",
                vm.blobs().len(),
                format_size(vm.total_blob_size()))),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title("Top Largest Blobs (Single Files)"));
    frame.render_widget(header, area);
}

fn render_list(frame: &mut Frame, vm: &BlobsViewModel, area: Rect) {
    let total_cumulative = vm.total_cumulative();
    let blobs = vm.blobs();

    let items: Vec<ListItem> = blobs
        .iter()
        .map(|blob| {
            let percent = if total_cumulative > 0 {
                blob.size as f64 / total_cumulative as f64 * 100.0
            } else {
                0.0
            };

            let bar_width = 12;
            let filled = ((percent / 100.0) * bar_width as f64) as usize;
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

            let date_str = format_timestamp(blob.first_date);

            // Truncate path for display
            let path_display = if blob.path.len() > 50 {
                format!("...{}", &blob.path[blob.path.len()-47..])
            } else {
                blob.path.clone()
            };

            // Truncate author
            let author_display = if blob.first_author.len() > 15 {
                format!("{}...", &blob.first_author[..12])
            } else {
                blob.first_author.clone()
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>10}", format_size(blob.size)), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(format!("[{}]", bar), Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled(format!("{:>7}", &hex::encode(&blob.oid)[..7]), Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::styled(format!("{:>15}", author_display), Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(format!("{:>10}", date_str), Style::default().fg(Color::White)),
                Span::raw("  "),
                Span::raw(path_display),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(vm.selected_index()));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!("Blobs ({} shown)", blobs.len())))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(Color::Yellow)), Span::raw(" nav  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)), Span::raw(" go to  "),
        Span::styled("l/Esc", Style::default().fg(Color::Yellow)), Span::raw(" tree  "),
        Span::styled("q", Style::default().fg(Color::Yellow)), Span::raw(" quit  |  "),
        Span::styled("SIZE", Style::default().fg(Color::Cyan)), Span::raw(" "),
        Span::styled("OID", Style::default().fg(Color::DarkGray)), Span::raw(" "),
        Span::styled("AUTHOR", Style::default().fg(Color::Yellow)), Span::raw(" "),
        Span::styled("DATE", Style::default().fg(Color::White)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, area);
}
