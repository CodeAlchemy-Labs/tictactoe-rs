//! Keyboard input handling.
//!
//! The translation from a raw `KeyCode` to an [`AppEvent`] is context
//! sensitive. In the auth screen, printable characters go to the form. In
//! the lobby, digits select a match and `t` opens the ranking. In a game,
//! digits play a move. The translator receives the current screen so it can
//! pick the right mapping.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use thiserror::Error;

use crate::app::AppEvent;
use crate::domain::screen::Screen;

/// Errors raised while polling for keyboard events.
#[derive(Debug, Error)]
pub enum InputError {
    /// The underlying terminal reported an I/O error.
    #[error("terminal input error: {0}")]
    Io(#[from] io::Error),
    /// The keyboard thread has been shut down.
    #[error("keyboard thread has been shut down")]
    Closed,
}

/// A high-level action produced by a key press.
pub enum KeyAction {
    /// Forward the given event to the state machine.
    Event(AppEvent),
    /// Ignore the key press.
    Ignored,
}

/// Blocks until a key is pressed and returns the corresponding action,
/// translated according to the screen currently shown.
///
/// # Errors
///
/// Returns [`InputError::Io`] on terminal failures.
pub fn read_key_action(screen: &Screen) -> Result<KeyAction, InputError> {
    loop {
        let event = event::read()?;
        let Event::Key(key) = event else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        return Ok(translate_key(key.code, screen));
    }
}

/// Translates a raw `KeyCode` into a [`KeyAction`] for the given screen.
///
/// Extracted from [`read_key_action`] so it can be unit-tested without a
/// terminal.
pub fn translate_key(code: KeyCode, screen: &Screen) -> KeyAction {
    match screen {
        Screen::Auth(_) => translate_auth_key(code),
        Screen::Spectating(_) => translate_spectating_key(code),
        _ => translate_default_key(code, screen),
    }
}

fn translate_auth_key(code: KeyCode) -> KeyAction {
    match code {
        KeyCode::Esc => KeyAction::Event(AppEvent::AuthCancel),
        KeyCode::Enter => KeyAction::Event(AppEvent::AuthSubmit),
        KeyCode::Tab => KeyAction::Event(AppEvent::AuthNextField),
        KeyCode::BackTab => KeyAction::Event(AppEvent::AuthPreviousField),
        KeyCode::Backspace => KeyAction::Event(AppEvent::AuthBackspace),
        KeyCode::F(2) => KeyAction::Event(AppEvent::AuthToggleMode),
        KeyCode::F(3) => KeyAction::Event(AppEvent::AuthToggleReveal),
        KeyCode::Char(character) => KeyAction::Event(AppEvent::AuthInput(character)),
        _ => KeyAction::Ignored,
    }
}

fn translate_spectating_key(code: KeyCode) -> KeyAction {
    match code {
        KeyCode::Esc => KeyAction::Event(AppEvent::LeaveSpectate),
        KeyCode::Char('q') => KeyAction::Event(AppEvent::Quit),
        _ => KeyAction::Ignored,
    }
}

fn translate_default_key(code: KeyCode, screen: &Screen) -> KeyAction {
    match code {
        KeyCode::Char('q') => KeyAction::Event(AppEvent::Quit),
        KeyCode::Esc => match screen {
            Screen::Ranking { .. } | Screen::Finished { .. } => {
                KeyAction::Event(AppEvent::BackToLobby)
            }
            Screen::Lobby {
                spectator_mode: true,
                ..
            } => KeyAction::Event(AppEvent::ToggleSpectatorMode),
            _ => KeyAction::Event(AppEvent::Quit),
        },
        KeyCode::Char('r') => KeyAction::Event(AppEvent::RefreshLobby),
        KeyCode::Char('t') => match screen {
            Screen::Lobby { .. } => KeyAction::Event(AppEvent::ShowRanking),
            _ => KeyAction::Ignored,
        },
        KeyCode::Char('c') => KeyAction::Event(AppEvent::CreateMatch),
        KeyCode::Char('l') => KeyAction::Event(AppEvent::LeaveMatch),
        KeyCode::Char('s') => match screen {
            Screen::Lobby { .. } => KeyAction::Event(AppEvent::ToggleSpectatorMode),
            _ => KeyAction::Ignored,
        },
        KeyCode::Char(digit @ '1'..='9') => {
            let value = digit as u8 - b'0';
            match screen {
                Screen::Lobby {
                    spectator_mode: true,
                    ..
                } => KeyAction::Event(AppEvent::SpectateAt((value - 1) as usize)),
                Screen::Lobby { .. } => {
                    KeyAction::Event(AppEvent::JoinMatchAt((value - 1) as usize))
                }
                Screen::InGame(_) => KeyAction::Event(AppEvent::PlayMove(value)),
                Screen::Connecting
                | Screen::Auth(_)
                | Screen::Ranking { .. }
                | Screen::Spectating(_)
                | Screen::Finished { .. }
                | Screen::Fatal(_) => KeyAction::Ignored,
            }
        }
        _ => KeyAction::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use common::domain::{Board, GameStatus, Player};
    use common::protocol::{MatchId, MatchSummary};

    use super::*;
    use crate::domain::auth_form::{AuthForm, AuthMode};
    use crate::domain::screen::{ActiveMatch, SpectatedMatch};

    fn lobby() -> Screen {
        Screen::Lobby {
            matches: vec![MatchSummary {
                id: MatchId::new(0),
                host: String::from("alice"),
                spectator_count: 0,
                is_full: false,
            }],
            spectator_mode: false,
        }
    }

    fn lobby_spectator_mode() -> Screen {
        Screen::Lobby {
            matches: vec![MatchSummary {
                id: MatchId::new(0),
                host: String::from("alice"),
                spectator_count: 0,
                is_full: false,
            }],
            spectator_mode: true,
        }
    }

    fn ranking() -> Screen {
        Screen::Ranking { entries: vec![] }
    }

    fn in_game() -> Screen {
        Screen::InGame(Box::new(ActiveMatch {
            id: MatchId::new(0),
            opponent: String::from("bob"),
            your_mark: Player::X,
            board: Board::new(),
            current_turn: Player::X,
            status: GameStatus::InProgress,
        }))
    }

    fn spectating() -> Screen {
        Screen::Spectating(Box::new(SpectatedMatch {
            id: MatchId::new(0),
            host_name: String::from("alice"),
            guest_name: String::from("bob"),
            board: Board::new(),
            current_turn: Player::X,
            status: GameStatus::InProgress,
            spectator_count: 1,
        }))
    }

    fn auth() -> Screen {
        Screen::Auth(Box::new(AuthForm::new(AuthMode::Login, None)))
    }

    #[test]
    fn digit_one_in_the_lobby_joins_the_first_match() {
        let action = translate_key(KeyCode::Char('1'), &lobby());
        assert!(matches!(action, KeyAction::Event(AppEvent::JoinMatchAt(0))));
    }

    #[test]
    fn digit_nine_in_the_lobby_joins_the_ninth_match() {
        let action = translate_key(KeyCode::Char('9'), &lobby());
        assert!(matches!(action, KeyAction::Event(AppEvent::JoinMatchAt(8))));
    }

    #[test]
    fn digit_in_spectator_mode_selects_a_match_to_spectate() {
        let action = translate_key(KeyCode::Char('1'), &lobby_spectator_mode());
        assert!(matches!(action, KeyAction::Event(AppEvent::SpectateAt(0))));
    }

    #[test]
    fn digit_one_in_a_game_plays_a_move() {
        let action = translate_key(KeyCode::Char('1'), &in_game());
        assert!(matches!(action, KeyAction::Event(AppEvent::PlayMove(1))));
    }

    #[test]
    fn digit_in_connecting_screen_is_ignored() {
        let action = translate_key(KeyCode::Char('1'), &Screen::Connecting);
        assert!(matches!(action, KeyAction::Ignored));
    }

    #[test]
    fn q_quits_from_anywhere_outside_auth() {
        let action = translate_key(KeyCode::Char('q'), &lobby());
        assert!(matches!(action, KeyAction::Event(AppEvent::Quit)));
    }

    #[test]
    fn unknown_key_is_ignored() {
        let action = translate_key(KeyCode::Char('z'), &lobby());
        assert!(matches!(action, KeyAction::Ignored));
    }

    #[test]
    fn printable_char_in_auth_is_forwarded_as_input() {
        let action = translate_key(KeyCode::Char('a'), &auth());
        assert!(matches!(action, KeyAction::Event(AppEvent::AuthInput('a'))));
    }

    #[test]
    fn tab_in_auth_moves_focus_forward() {
        let action = translate_key(KeyCode::Tab, &auth());
        assert!(matches!(action, KeyAction::Event(AppEvent::AuthNextField)));
    }

    #[test]
    fn back_tab_in_auth_moves_focus_backwards() {
        let action = translate_key(KeyCode::BackTab, &auth());
        assert!(matches!(
            action,
            KeyAction::Event(AppEvent::AuthPreviousField)
        ));
    }

    #[test]
    fn esc_in_auth_cancels_the_screen() {
        let action = translate_key(KeyCode::Esc, &auth());
        assert!(matches!(action, KeyAction::Event(AppEvent::AuthCancel)));
    }

    #[test]
    fn f2_in_auth_toggles_the_mode() {
        let action = translate_key(KeyCode::F(2), &auth());
        assert!(matches!(action, KeyAction::Event(AppEvent::AuthToggleMode)));
    }

    #[test]
    fn f3_in_auth_toggles_password_reveal() {
        let action = translate_key(KeyCode::F(3), &auth());
        assert!(matches!(
            action,
            KeyAction::Event(AppEvent::AuthToggleReveal)
        ));
    }

    #[test]
    fn t_in_the_lobby_opens_the_ranking() {
        let action = translate_key(KeyCode::Char('t'), &lobby());
        assert!(matches!(action, KeyAction::Event(AppEvent::ShowRanking)));
    }

    #[test]
    fn t_outside_the_lobby_is_ignored() {
        let action = translate_key(KeyCode::Char('t'), &in_game());
        assert!(matches!(action, KeyAction::Ignored));
    }

    #[test]
    fn esc_on_the_ranking_screen_returns_to_the_lobby() {
        let action = translate_key(KeyCode::Esc, &ranking());
        assert!(matches!(action, KeyAction::Event(AppEvent::BackToLobby)));
    }

    #[test]
    fn esc_outside_the_ranking_screen_quits() {
        let action = translate_key(KeyCode::Esc, &lobby());
        assert!(matches!(action, KeyAction::Event(AppEvent::Quit)));
    }

    #[test]
    fn esc_on_the_finished_screen_returns_to_the_lobby() {
        let screen = Screen::Finished {
            board: Board::new(),
            status: GameStatus::Draw,
            winner_name: None,
        };
        let action = translate_key(KeyCode::Esc, &screen);
        assert!(matches!(action, KeyAction::Event(AppEvent::BackToLobby)));
    }

    #[test]
    fn s_in_the_lobby_toggles_spectator_mode() {
        let action = translate_key(KeyCode::Char('s'), &lobby());
        assert!(matches!(
            action,
            KeyAction::Event(AppEvent::ToggleSpectatorMode)
        ));
    }

    #[test]
    fn esc_in_spectator_mode_cancels_the_mode() {
        let action = translate_key(KeyCode::Esc, &lobby_spectator_mode());
        assert!(matches!(
            action,
            KeyAction::Event(AppEvent::ToggleSpectatorMode)
        ));
    }

    #[test]
    fn esc_on_the_spectating_screen_leaves_spectate() {
        let action = translate_key(KeyCode::Esc, &spectating());
        assert!(matches!(action, KeyAction::Event(AppEvent::LeaveSpectate)));
    }

    #[test]
    fn q_on_the_spectating_screen_quits() {
        let action = translate_key(KeyCode::Char('q'), &spectating());
        assert!(matches!(action, KeyAction::Event(AppEvent::Quit)));
    }

    #[test]
    fn digits_on_the_spectating_screen_are_ignored() {
        let action = translate_key(KeyCode::Char('1'), &spectating());
        assert!(matches!(action, KeyAction::Ignored));
    }
}
