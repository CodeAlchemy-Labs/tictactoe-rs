//! Rendering for the terminal UI.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use common::domain::{Cell, GameStatus, Player};

use crate::app::AppState;
use crate::domain::screen::{ActiveMatch, Screen};

/// Renders the whole UI for the current state.
pub fn render(frame: &mut Frame<'_>, state: &AppState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, chunks[0], state);
    render_body(frame, chunks[1], state);
    render_footer(frame, chunks[2], state);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let title = format!(" tictactoe-rs | {} | {} ", state.display_name, state.screen.title());
    let paragraph = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_body(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    match &state.screen {
        Screen::Connecting => {
            frame.render_widget(
                Paragraph::new("Connecting to the server...")
                    .block(Block::default().borders(Borders::ALL))
                    .alignment(Alignment::Center),
                area,
            );
        }
        Screen::Lobby { matches } => render_lobby(frame, area, matches),
        Screen::InGame(active) => render_game(frame, area, active),
        Screen::Finished { board, status } => render_finished(frame, area, *board, *status),
        Screen::Fatal(message) => {
            frame.render_widget(
                Paragraph::new(message.as_str())
                    .block(Block::default().borders(Borders::ALL).title("Error"))
                    .wrap(Wrap { trim: true }),
                area,
            );
        }
    }
}

fn render_lobby(frame: &mut Frame<'_>, area: Rect, matches: &[common::protocol::MatchSummary]) {
    let items: Vec<ListItem<'_>> = if matches.is_empty() {
        vec![ListItem::new("no open matches; press c to create one")]
    } else {
        matches
            .iter()
            .enumerate()
            .map(|(index, summary)| {
                ListItem::new(format!("[{}] {} (host: {})", index + 1, summary.id, summary.host))
            })
            .collect()
    };
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Lobby"));
    frame.render_widget(list, area);
}

fn render_game(frame: &mut Frame<'_>, area: Rect, active: &ActiveMatch) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(31), Constraint::Min(20)])
        .split(area);

    let board_lines = build_board_lines(active);
    let board = Paragraph::new(board_lines)
        .block(Block::default().borders(Borders::ALL).title("Board"))
        .alignment(Alignment::Center);
    frame.render_widget(board, columns[0]);

    let side_text = vec![
        Line::from(format!("match:   {}", active.id)),
        Line::from(format!("opponent: {}", active.opponent)),
        Line::from(format!("you:      {:?}", active.your_mark)),
        Line::from(format!("turn:     {:?}", active.current_turn)),
        Line::from(""),
        Line::from("keys 1-9 map to cells in row-major order:"),
        Line::from("  1 | 2 | 3"),
        Line::from("  4 | 5 | 6"),
        Line::from("  7 | 8 | 9"),
    ];
    let side = Paragraph::new(side_text)
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .wrap(Wrap { trim: true });
    frame.render_widget(side, columns[1]);
}

fn render_finished(frame: &mut Frame<'_>, area: Rect, board: common::domain::Board, status: GameStatus) {
    let outcome = match status {
        GameStatus::Won(player) => format!("{player:?} wins"),
        GameStatus::Draw => String::from("draw"),
        GameStatus::InProgress => String::from("in progress"),
    };
    let mut lines = vec![Line::from(outcome), Line::from("")];
    lines.extend(build_board_lines_from(&board));
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Result"))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let paragraph = Paragraph::new(state.status.clone())
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn build_board_lines(active: &ActiveMatch) -> Vec<Line<'static>> {
    build_board_lines_from(&active.board)
}

fn build_board_lines_from(board: &common::domain::Board) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(5);
    for row in 0..3u8 {
        let mut spans = Vec::with_capacity(5);
        for column in 0..3u8 {
            let index = row * 3 + column;
            let position = common::domain::Position::new(index).expect("index is in range");
            let cell = board.get(position);
            spans.push(Span::styled(
                cell_glyph(cell),
                cell_style(cell),
            ));
            if column < 2 {
                spans.push(Span::raw(" | "));
            }
        }
        lines.push(Line::from(spans));
        if row < 2 {
            lines.push(Line::from("---+---+---"));
        }
    }
    lines
}

const fn cell_glyph(cell: Cell) -> &'static str {
    match cell {
        Cell::Empty => " ",
        Cell::Occupied(Player::X) => "X",
        Cell::Occupied(Player::O) => "O",
    }
}

fn cell_style(cell: Cell) -> Style {
    match cell {
        Cell::Occupied(Player::X) => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        Cell::Occupied(Player::O) => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        Cell::Empty => Style::default(),
    }
}