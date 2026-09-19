//! Rendering for the terminal UI.

use crate::domain::{ConnectionField, ConnectionForm};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Wrap};

use common::domain::{Cell, GameStatus, Player, RankingEntry};

use crate::app::AppState;
use crate::domain::auth_form::{AuthField, AuthForm, AuthMode};
use crate::domain::screen::{ActiveMatch, Screen, SpectatedMatch};

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
    let title = format!(
        " tictactoe-rs | {} | {} ",
        state.display_name,
        state.screen.title()
    );
    let paragraph = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_body(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    match &state.screen {
        Screen::Connection => {
            render_connection(frame, area, &state.connection_form);
        }
        Screen::Connecting => {
            frame.render_widget(
                Paragraph::new("Connecting to the server...")
                    .block(Block::default().borders(Borders::ALL))
                    .alignment(Alignment::Center),
                area,
            );
        }
        Screen::Auth(form) => render_auth(frame, area, form),
        Screen::Lobby { spectator_mode, .. } => {
            let visible = state.screen.visible_matches();
            render_lobby(frame, area, &visible, *spectator_mode);
        }
        Screen::Ranking { entries } => render_ranking(frame, area, entries),
        Screen::InGame(active) => render_game(frame, area, active),
        Screen::Spectating(active) => render_spectating(frame, area, active),
        Screen::Finished {
            board,
            status,
            winner_name,
        } => render_finished(frame, area, *board, *status, winner_name.as_deref()),
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

fn render_lobby(
    frame: &mut Frame<'_>,
    area: Rect,
    matches: &[&common::protocol::MatchSummary],
    spectator_mode: bool,
) {
    let columns = if spectator_mode {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5)])
            .split(area)
    };

    if spectator_mode {
        let banner = Paragraph::new("Spectator mode: press 1-9 to watch a match, Esc to cancel")
            .block(Block::default().borders(Borders::ALL))
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);
        frame.render_widget(banner, columns[0]);
    }

    let body_area = if spectator_mode {
        columns[1]
    } else {
        columns[0]
    };

    let items: Vec<ListItem<'_>> = if matches.is_empty() {
        let message = if spectator_mode {
            "no matches to spectate; press r to refresh"
        } else {
            "no open matches; press c to create one"
        };
        vec![ListItem::new(message)]
    } else {
        matches
            .iter()
            .enumerate()
            .map(|(index, summary)| {
                let spectators = if summary.spectator_count > 0 {
                    format!(", {} watching", summary.spectator_count)
                } else {
                    String::new()
                };
                let status = if summary.is_full { ", in progress" } else { "" };
                ListItem::new(format!(
                    "[{}] {} (host: {}{}{})",
                    index + 1,
                    summary.id,
                    summary.host,
                    spectators,
                    status
                ))
            })
            .collect()
    };
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Lobby"));
    frame.render_widget(list, body_area);
}

fn render_ranking(frame: &mut Frame<'_>, area: Rect, entries: &[RankingEntry]) {
    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new("no wins recorded yet")
                .block(Block::default().borders(Borders::ALL).title("Top players"))
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let rows: Vec<Row<'_>> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let style = if index == 0 {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![
                format!("{}", index + 1),
                entry.username.to_string(),
                entry.name.clone(),
                format!("{}", entry.wins),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Min(12),
        Constraint::Min(16),
        Constraint::Length(6),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["#", "Username", "Name", "Wins"])
                .style(Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
        )
        .block(Block::default().borders(Borders::ALL).title("Top players"))
        .column_spacing(2);
    frame.render_widget(table, area);
}

fn render_game(frame: &mut Frame<'_>, area: Rect, active: &ActiveMatch) {
    // The board has a fixed visual width of 13 cells (2 borders plus the
    // 11-wide interior: 3 marks and 2 separators of " | " per row). The
    // status panel takes whatever is left, with a minimum of 20 columns.
    // On narrow terminals the min on the status side is what yields first,
    // so the board stays intact.
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(13), Constraint::Min(20)])
        .split(area);

    let board_lines = build_board_lines_from(&active.board);
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

fn render_spectating(frame: &mut Frame<'_>, area: Rect, active: &SpectatedMatch) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(31), Constraint::Min(20)])
        .split(area);

    let board_lines = build_board_lines_from(&active.board);
    let board = Paragraph::new(board_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Board (read-only)"),
        )
        .alignment(Alignment::Center);
    frame.render_widget(board, columns[0]);

    let side_text = vec![
        Line::from(format!("match:       {}", active.id)),
        Line::from(format!("host:        {}", active.host_name)),
        Line::from(format!("guest:       {}", active.guest_name)),
        Line::from(format!("turn:        {:?}", active.current_turn)),
        Line::from(format!("status:      {:?}", active.status)),
        Line::from(format!("spectators:  {}", active.spectator_count)),
        Line::from(""),
        Line::from("you are observing this match."),
        Line::from("Esc to leave."),
    ];
    let side = Paragraph::new(side_text)
        .block(Block::default().borders(Borders::ALL).title("Spectating"))
        .wrap(Wrap { trim: true });
    frame.render_widget(side, columns[1]);
}

fn render_finished(
    frame: &mut Frame<'_>,
    area: Rect,
    board: common::domain::Board,
    status: GameStatus,
    winner_name: Option<&str>,
) {
    let outcome = match status {
        GameStatus::Won(player) => {
            let name = winner_name.unwrap_or(match player {
                Player::X => "X",
                Player::O => "O",
            });
            format!("The player {name} won")
        }
        GameStatus::Draw => String::from("The game ended in a draw"),
        GameStatus::InProgress => String::from("in progress"),
    };
    let mut lines = vec![Line::from(outcome), Line::from("")];
    lines.extend(build_board_lines_from(&board));
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Result"))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_auth(frame: &mut Frame<'_>, area: Rect, form: &AuthForm) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    let title = form.mode.title();
    let mut lines: Vec<Line<'static>> = Vec::new();
    if form.mode == AuthMode::Register {
        lines.push(field_line(
            "Name",
            &form.name,
            form.focused == AuthField::Name,
        ));
    }
    lines.push(field_line(
        "Username",
        &form.username,
        form.focused == AuthField::Username,
    ));
    if form.mode == AuthMode::Register {
        lines.push(field_line("Age", &form.age, form.focused == AuthField::Age));
    }
    let password_rendered = if form.reveal_password {
        form.password.clone()
    } else {
        "*".repeat(form.password.chars().count())
    };
    lines.push(field_line(
        "Password",
        &password_rendered,
        form.focused == AuthField::Password,
    ));

    let fields = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(fields, rows[0]);

    let error_text = form.error.clone().unwrap_or_default();
    let error = Paragraph::new(error_text)
        .block(Block::default().borders(Borders::ALL).title("Message"))
        .style(Style::default().fg(Color::Red));
    frame.render_widget(error, rows[1]);

    let help = Paragraph::new(
        "Tab: next  Shift-Tab: previous  Enter: submit  F2: toggle mode  F3: reveal  Esc: cancel",
    )
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, rows[2]);
}

fn field_line(label: &str, value: &str, focused: bool) -> Line<'static> {
    let style = if focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let marker = if focused { "> " } else { "  " };
    Line::from(vec![
        Span::styled(marker.to_string(), style),
        Span::styled(format!("{label:>9}: "), style),
        Span::styled(value.to_string(), style),
    ])
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let paragraph = Paragraph::new(state.status.clone())
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn build_board_lines_from(board: &common::domain::Board) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(5);
    for row in 0..3u8 {
        let mut spans = Vec::with_capacity(5);
        for column in 0..3u8 {
            let index = row * 3 + column;
            let position = common::domain::Position::new(index).expect("index is in range");
            let cell = board.get(position);
            spans.push(Span::styled(cell_glyph(cell), cell_style(cell)));
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
        Cell::Occupied(Player::X) => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        Cell::Occupied(Player::O) => Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
        Cell::Empty => Style::default(),
    }
}

fn render_connection(frame: &mut Frame<'_>, area: Rect, form: &ConnectionForm) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    let mut lines: Vec<Line<'static>> = Vec::new();

    let scheme_label = if form.use_tls { "wss://" } else { "ws://" };

    let host_line = format!("{}{}", scheme_label, form.host);
    lines.push(field_line(
        "Server",
        &host_line,
        form.focus == ConnectionField::Server,
    ));

    lines.push(field_line(
        "Name",
        &form.guest_name,
        form.focus == ConnectionField::GuestName,
    ));

    let tls_value = if form.use_tls { "enabled" } else { "disabled" };
    lines.push(field_line(
        "TLS",
        tls_value,
        form.focus == ConnectionField::Tls,
    ));

    let fields =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Connection"));
    frame.render_widget(fields, rows[0]);

    let error_text = form.error.clone().unwrap_or_default();
    let error = Paragraph::new(error_text)
        .block(Block::default().borders(Borders::ALL).title("Message"))
        .style(Style::default().fg(Color::Red));
    frame.render_widget(error, rows[1]);

    let help =
        Paragraph::new("Tab: next  Shift-Tab: previous  Enter: submit  F3: toggle TLS  Esc: quit")
            .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, rows[2]);
}
