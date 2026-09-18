//! Unit tests for [`LobbyService`].
//!
//! These tests exercise the service directly, without going through a
//! WebSocket connection. The integration tests in `tests/websocket.rs`
//! cover the wire-level behavior.

use argon2::Params;
use common::domain::{GameStatus, Player, Position};
use common::protocol::ErrorCode;

use super::*;

fn fast_lobby() -> LobbyService {
    let params = Params::new(8, 1, 1, None).expect("test parameters are within range");
    LobbyService::with_auth(Arc::new(AuthService::with_params(params)))
}

fn lobby_with_client() -> (
    LobbyService,
    ClientId,
    mpsc::UnboundedReceiver<ServerMessage>,
) {
    let lobby = fast_lobby();
    let (tx, rx) = mpsc::unbounded_channel();
    let id = lobby.register_client(tx);
    (lobby, id, rx)
}

async fn authenticate(lobby: &LobbyService, client: ClientId, username: &str) {
    lobby
        .register_user(
            client,
            format!("{username} name"),
            username.to_string(),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
}

#[test]
fn register_increments_session_count() {
    let (lobby, _, _) = lobby_with_client();
    assert_eq!(lobby.session_count(), 1);
}

#[test]
fn hello_sends_welcome() {
    let (lobby, id, mut rx) = lobby_with_client();
    lobby.hello(id, "alice");
    match rx.try_recv().unwrap() {
        ServerMessage::Welcome { display_name, .. } => assert_eq!(display_name, "alice"),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn empty_display_name_is_rejected() {
    let (lobby, id, mut rx) = lobby_with_client();
    lobby.hello(id, "   ");
    match rx.try_recv().unwrap() {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::InvalidDisplayName),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn create_match_requires_hello() {
    let (lobby, id, mut rx) = lobby_with_client();
    lobby.create_match(id);
    match rx.try_recv().unwrap() {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::InvalidState),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn create_match_requires_authentication() {
    let (lobby, id, mut rx) = lobby_with_client();
    lobby.hello(id, "guest");
    let _ = rx.try_recv();
    lobby.create_match(id);
    match rx.try_recv().unwrap() {
        ServerMessage::Error { code, .. } => {
            assert_eq!(code, ErrorCode::AuthenticationRequired);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn authenticated_client_can_create_a_match() {
    let (lobby, id, mut rx) = lobby_with_client();
    authenticate(&lobby, id, "alice_99").await;
    let _ = rx.try_recv(); // Registered
    lobby.create_match(id);
    match rx.try_recv().unwrap() {
        ServerMessage::MatchCreated { .. } => {}
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn joining_a_match_notifies_both_players() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    let _ = guest_rx.try_recv();

    lobby.create_match(host);
    let match_id = match host_rx.try_recv().unwrap() {
        ServerMessage::MatchCreated { match_id } => match_id,
        other => panic!("unexpected: {other:?}"),
    };
    lobby.join_match(guest, match_id);

    match host_rx.try_recv().unwrap() {
        ServerMessage::MatchReady {
            your_mark,
            opponent,
            ..
        } => {
            assert_eq!(your_mark, Player::X);
            assert_eq!(opponent, "guest_99 name");
        }
        other => panic!("unexpected: {other:?}"),
    }
    match guest_rx.try_recv().unwrap() {
        ServerMessage::MatchReady {
            your_mark,
            opponent,
            ..
        } => {
            assert_eq!(your_mark, Player::O);
            assert_eq!(opponent, "host_99 name");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn playing_a_full_game_ends_in_a_win_and_records_it() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    let _ = guest_rx.try_recv();
    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!("first message must be MatchCreated")
    };
    lobby.join_match(guest, match_id);
    let _ = host_rx.try_recv();
    let _ = guest_rx.try_recv();

    for (client, pos) in [(host, 0u8), (guest, 3), (host, 1), (guest, 4), (host, 2)] {
        lobby.make_move(client, Position::new(pos).unwrap());
    }

    let mut saw_match_over = false;
    while let Ok(message) = host_rx.try_recv() {
        if let ServerMessage::MatchOver {
            status,
            winner_name,
            ..
        } = message
        {
            assert_eq!(status, GameStatus::Won(Player::X));
            assert_eq!(winner_name.as_deref(), Some("host_99 name"));
            saw_match_over = true;
        }
    }
    assert!(saw_match_over);

    let ranking = lobby.ranking().top(crate::application::ranking::TOP_N);
    assert_eq!(ranking.len(), 1);
    assert_eq!(ranking[0].username.as_str(), "host_99");
    assert_eq!(ranking[0].wins, 1);

    // The match was removed from the lobby.
    assert_eq!(lobby.match_count(), 0);
}

#[tokio::test]
async fn disconnect_removes_the_session_and_the_open_match() {
    let lobby = fast_lobby();
    let (tx, _rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(tx);
    authenticate(&lobby, host, "host_99").await;
    lobby.create_match(host);
    assert_eq!(lobby.session_count(), 1);
    assert_eq!(lobby.match_count(), 1);

    lobby.disconnect(host);
    assert_eq!(lobby.session_count(), 0);
    assert_eq!(lobby.match_count(), 0);
}

#[tokio::test]
async fn disconnect_notifies_the_opponent() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    let _ = guest_rx.try_recv();
    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!("first message must be MatchCreated")
    };
    lobby.join_match(guest, match_id);
    let _ = host_rx.try_recv();
    let _ = guest_rx.try_recv();

    lobby.disconnect(host);
    match guest_rx.try_recv().unwrap() {
        ServerMessage::MatchAbandoned { .. } => {}
        other => panic!("unexpected: {other:?}"),
    }
    assert_eq!(lobby.session_count(), 1);
    assert_eq!(lobby.match_count(), 0);
}

#[tokio::test]
async fn register_marks_the_session_as_authenticated() {
    let lobby = fast_lobby();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = lobby.register_client(tx);
    lobby
        .register_user(
            client,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    match rx.try_recv().unwrap() {
        ServerMessage::Registered { profile } => {
            assert_eq!(profile.username.as_str(), "alice_99");
        }
        other => panic!("unexpected: {other:?}"),
    }
    assert!(lobby.is_authenticated(client));
}

#[tokio::test]
async fn register_rejects_short_password() {
    let lobby = fast_lobby();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = lobby.register_client(tx);
    lobby
        .register_user(
            client,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("short"),
        )
        .await;
    match rx.try_recv().unwrap() {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::PasswordTooShort);
        }
        other => panic!("unexpected: {other:?}"),
    }
    assert!(!lobby.is_authenticated(client));
}

#[tokio::test]
async fn register_rejects_duplicate_username() {
    let lobby = fast_lobby();
    let (tx1, mut rx1) = mpsc::unbounded_channel();
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    let client1 = lobby.register_client(tx1);
    let client2 = lobby.register_client(tx2);
    lobby
        .register_user(
            client1,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    let _ = rx1.try_recv();
    lobby
        .register_user(
            client2,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("otherpassword"),
        )
        .await;
    match rx2.try_recv().unwrap() {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::UsernameTaken);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn login_succeeds_after_registration() {
    let lobby = fast_lobby();
    let (tx1, mut rx1) = mpsc::unbounded_channel();
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    let client1 = lobby.register_client(tx1);
    let client2 = lobby.register_client(tx2);
    lobby
        .register_user(
            client1,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    let _ = rx1.try_recv();

    // Close the first connection so the account is no longer active.
    lobby.disconnect(client1);

    lobby
        .login_user(
            client2,
            String::from("alice_99"),
            String::from("hunter2hunter2"),
        )
        .await;
    match rx2.try_recv().unwrap() {
        ServerMessage::LoginSucceeded { profile } => {
            assert_eq!(profile.username.as_str(), "alice_99");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn login_rejects_wrong_password() {
    let lobby = fast_lobby();
    let (tx1, mut rx1) = mpsc::unbounded_channel();
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    let client1 = lobby.register_client(tx1);
    let client2 = lobby.register_client(tx2);
    lobby
        .register_user(
            client1,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    let _ = rx1.try_recv();

    // Close the first session so the account is no longer active and the
    // login path can reach password verification.
    lobby.disconnect(client1);

    lobby
        .login_user(
            client2,
            String::from("alice_99"),
            String::from("wrongpassword"),
        )
        .await;
    match rx2.try_recv().unwrap() {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::InvalidCredentials);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn already_logged_in_takes_precedence_over_password_check() {
    // The active-session check runs before password verification. From the
    // outside, a second login attempt against an active account is rejected
    // with `AlreadyLoggedIn` regardless of whether the password is correct.
    // This test documents that ordering.
    let lobby = fast_lobby();
    let (tx1, mut rx1) = mpsc::unbounded_channel();
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    let client1 = lobby.register_client(tx1);
    let client2 = lobby.register_client(tx2);
    lobby
        .register_user(
            client1,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    let _ = rx1.try_recv();

    lobby
        .login_user(
            client2,
            String::from("alice_99"),
            String::from("wrongpassword"),
        )
        .await;
    match rx2.try_recv().unwrap() {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::AlreadyLoggedIn);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn double_registration_is_rejected() {
    let lobby = fast_lobby();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = lobby.register_client(tx);
    lobby
        .register_user(
            client,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    let _ = rx.try_recv();
    lobby
        .register_user(
            client,
            String::from("Bob"),
            String::from("bob_77"),
            25,
            String::from("hunter2hunter2"),
        )
        .await;
    match rx.try_recv().unwrap() {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::AlreadyAuthenticated);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn register_rejects_invalid_username() {
    let lobby = fast_lobby();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = lobby.register_client(tx);
    lobby
        .register_user(
            client,
            String::from("Alice"),
            String::from("a!"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    match rx.try_recv().unwrap() {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::UsernameInvalid);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn register_rejects_invalid_age() {
    let lobby = fast_lobby();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = lobby.register_client(tx);
    lobby
        .register_user(
            client,
            String::from("Alice"),
            String::from("alice_99"),
            91,
            String::from("hunter2hunter2"),
        )
        .await;
    match rx.try_recv().unwrap() {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::AgeOutOfRange);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn login_rejects_when_account_is_already_active() {
    let lobby = fast_lobby();
    let (tx1, mut rx1) = mpsc::unbounded_channel();
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    let client1 = lobby.register_client(tx1);
    let client2 = lobby.register_client(tx2);
    lobby
        .register_user(
            client1,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    let _ = rx1.try_recv();
    // The first session is still active; the second login must fail.

    lobby
        .login_user(
            client2,
            String::from("alice_99"),
            String::from("hunter2hunter2"),
        )
        .await;
    match rx2.try_recv().unwrap() {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::AlreadyLoggedIn);
        }
        other => panic!("unexpected: {other:?}"),
    }
    assert!(!lobby.is_authenticated(client2));
}

#[tokio::test]
async fn login_succeeds_after_the_first_session_disconnects() {
    let lobby = fast_lobby();
    let (tx1, mut rx1) = mpsc::unbounded_channel();
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    let client1 = lobby.register_client(tx1);
    let client2 = lobby.register_client(tx2);
    lobby
        .register_user(
            client1,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    let _ = rx1.try_recv();

    // Close the first connection; the active entry must be released.
    lobby.disconnect(client1);

    lobby
        .login_user(
            client2,
            String::from("alice_99"),
            String::from("hunter2hunter2"),
        )
        .await;
    match rx2.try_recv().unwrap() {
        ServerMessage::LoginSucceeded { profile } => {
            assert_eq!(profile.username.as_str(), "alice_99");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn register_rejects_username_taken_by_an_active_session() {
    // The username already exists, so the register path returns
    // UsernameTaken even though the account is currently signed in.
    // AlreadyLoggedIn is reserved for the login path.
    let lobby = fast_lobby();
    let (tx1, mut rx1) = mpsc::unbounded_channel();
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    let client1 = lobby.register_client(tx1);
    let client2 = lobby.register_client(tx2);
    lobby
        .register_user(
            client1,
            String::from("Alice"),
            String::from("alice_99"),
            30,
            String::from("hunter2hunter2"),
        )
        .await;
    let _ = rx1.try_recv();

    lobby
        .register_user(
            client2,
            String::from("Alice Impostor"),
            String::from("alice_99"),
            25,
            String::from("hunter2hunter2"),
        )
        .await;
    match rx2.try_recv().unwrap() {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::UsernameTaken);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn draws_do_not_record_wins() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    let _ = guest_rx.try_recv();
    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!("first message must be MatchCreated")
    };
    lobby.join_match(guest, match_id);
    let _ = host_rx.try_recv();
    let _ = guest_rx.try_recv();

    // X: 0, 2, 3, 7, 8. O: 1, 4, 5, 6. Five X moves, four O moves, no line.
    for (client, pos) in [
        (host, 0u8),
        (guest, 1),
        (host, 2),
        (guest, 4),
        (host, 3),
        (guest, 5),
        (host, 7),
        (guest, 6),
        (host, 8),
    ] {
        lobby.make_move(client, Position::new(pos).unwrap());
    }

    let mut saw_draw = false;
    while let Ok(message) = host_rx.try_recv() {
        if let ServerMessage::MatchOver {
            status,
            winner_name,
            ..
        } = message
        {
            assert_eq!(status, GameStatus::Draw);
            assert_eq!(winner_name, None);
            saw_draw = true;
        }
    }
    assert!(saw_draw);

    assert!(lobby.ranking().is_empty());
    assert_eq!(lobby.match_count(), 0);
}

#[tokio::test]
async fn multiple_wins_accumulate_in_the_ranking() {
    let lobby = fast_lobby();
    let champ_username = "champ_99";

    // Round 1: register the champion and win.
    {
        let (host_tx, mut host_rx) = mpsc::unbounded_channel();
        let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
        let host = lobby.register_client(host_tx);
        let guest = lobby.register_client(guest_tx);

        lobby
            .register_user(
                host,
                String::from("Champion"),
                champ_username.to_string(),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = host_rx.try_recv();
        lobby
            .register_user(
                guest,
                String::from("Rival One"),
                String::from("rival_one"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = guest_rx.try_recv();

        lobby.create_match(host);
        let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
            unreachable!("first message must be MatchCreated")
        };
        lobby.join_match(guest, match_id);
        let _ = host_rx.try_recv();
        let _ = guest_rx.try_recv();

        for (client, pos) in [(host, 0u8), (guest, 3), (host, 1), (guest, 4), (host, 2)] {
            lobby.make_move(client, Position::new(pos).unwrap());
        }

        lobby.disconnect(host);
        lobby.disconnect(guest);
    }

    // Round 2: log in the champion (already registered) and win again.
    {
        let (host_tx, mut host_rx) = mpsc::unbounded_channel();
        let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
        let host = lobby.register_client(host_tx);
        let guest = lobby.register_client(guest_tx);

        lobby
            .login_user(
                host,
                champ_username.to_string(),
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = host_rx.try_recv();
        lobby
            .register_user(
                guest,
                String::from("Rival Two"),
                String::from("rival_two"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = guest_rx.try_recv();

        lobby.create_match(host);
        let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
            unreachable!("first message must be MatchCreated")
        };
        lobby.join_match(guest, match_id);
        let _ = host_rx.try_recv();
        let _ = guest_rx.try_recv();

        for (client, pos) in [(host, 0u8), (guest, 3), (host, 1), (guest, 4), (host, 2)] {
            lobby.make_move(client, Position::new(pos).unwrap());
        }

        lobby.disconnect(host);
        lobby.disconnect(guest);
    }

    let ranking = lobby.ranking().top(crate::application::ranking::TOP_N);
    assert_eq!(ranking.len(), 1);
    assert_eq!(ranking[0].username.as_str(), champ_username);
    assert_eq!(ranking[0].wins, 2);
}

#[tokio::test]
async fn after_a_win_the_player_can_start_a_new_match() {
    // Regression test: previously the server left `current_match` set on
    // the players after a match ended, so creating a new match failed with
    // "already in a match".
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    let _ = guest_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!("first message must be MatchCreated")
    };
    lobby.join_match(guest, match_id);
    let _ = host_rx.try_recv();
    let _ = guest_rx.try_recv();

    for (client, pos) in [(host, 0u8), (guest, 3), (host, 1), (guest, 4), (host, 2)] {
        lobby.make_move(client, Position::new(pos).unwrap());
    }
    // Drain the broadcast messages so the next assertion is not polluted.
    while host_rx.try_recv().is_ok() {}
    while guest_rx.try_recv().is_ok() {}

    // Both players must be free again.
    lobby.create_match(host);
    match host_rx.try_recv().unwrap() {
        ServerMessage::MatchCreated { .. } => {}
        other => panic!("expected MatchCreated, got {other:?}"),
    }

    // Clean up: leave the freshly created match so the guest can also
    // create one.
    lobby.leave_match(host);
    while host_rx.try_recv().is_ok() {}
    while guest_rx.try_recv().is_ok() {}

    lobby.create_match(guest);
    match guest_rx.try_recv().unwrap() {
        ServerMessage::MatchCreated { .. } => {}
        other => panic!("expected MatchCreated, got {other:?}"),
    }
}

#[tokio::test]
async fn host_cannot_join_its_own_match() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!("first message must be MatchCreated")
    };

    // The host tries to join its own match. The server must reject it.
    lobby.join_match(host, match_id);
    match host_rx.try_recv().unwrap() {
        ServerMessage::Error { code, .. } => {
            assert_eq!(code, ErrorCode::CannotJoinOwnMatch);
        }
        other => panic!("expected CannotJoinOwnMatch, got {other:?}"),
    }
}

#[tokio::test]
async fn guest_leaving_destroys_the_match_and_frees_the_host() {
    // Regression test: previously, when the guest left, the match survived
    // with `guest = None`. The host could then join its own match as guest,
    // and afterwards could not create a new one with "already in a match".
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    let _ = guest_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!("first message must be MatchCreated")
    };
    lobby.join_match(guest, match_id);
    let _ = host_rx.try_recv();
    let _ = guest_rx.try_recv();
    assert_eq!(lobby.match_count(), 1);

    // The guest leaves.
    lobby.leave_match(guest);
    match host_rx.try_recv().unwrap() {
        ServerMessage::MatchAbandoned { .. } => {}
        other => panic!("expected MatchAbandoned, got {other:?}"),
    }
    assert_eq!(lobby.match_count(), 0);

    // The host is free to create another match.
    lobby.create_match(host);
    match host_rx.try_recv().unwrap() {
        ServerMessage::MatchCreated { .. } => {}
        other => panic!("expected MatchCreated, got {other:?}"),
    }
}

#[tokio::test]
async fn spectator_receives_the_started_snapshot() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
    let (spec_tx, mut spec_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    let spectator = lobby.register_client(spec_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    let _ = guest_rx.try_recv();
    // The spectator is authenticated but does not play.
    authenticate(&lobby, spectator, "watch_99").await;
    let _ = spec_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!("first message must be MatchCreated")
    };
    lobby.join_match(guest, match_id);
    let _ = host_rx.try_recv();
    let _ = guest_rx.try_recv();

    lobby.spectate(spectator, match_id);

    match spec_rx.try_recv().unwrap() {
        ServerMessage::SpectateStarted {
            match_id: got_id,
            host_name,
            guest_name,
            spectator_count,
            status,
            ..
        } => {
            assert_eq!(got_id, match_id);
            assert_eq!(host_name, "host_99 name");
            assert_eq!(guest_name, "guest_99 name");
            assert_eq!(spectator_count, 1);
            assert_eq!(status, GameStatus::InProgress);
        }
        other => panic!("expected SpectateStarted, got {other:?}"),
    }

    // The players were notified that a spectator joined.
    match host_rx.try_recv().unwrap() {
        ServerMessage::SpectatorJoined {
            username,
            spectator_count,
        } => {
            assert_eq!(username, "watch_99 name");
            assert_eq!(spectator_count, 1);
        }
        other => panic!("expected SpectatorJoined, got {other:?}"),
    }
    match guest_rx.try_recv().unwrap() {
        ServerMessage::SpectatorJoined {
            username,
            spectator_count,
        } => {
            assert_eq!(username, "watch_99 name");
            assert_eq!(spectator_count, 1);
        }
        other => panic!("expected SpectatorJoined, got {other:?}"),
    }
}

#[tokio::test]
async fn spectator_receives_board_updates() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
    let (spec_tx, mut spec_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    let spectator = lobby.register_client(spec_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    let _ = guest_rx.try_recv();
    authenticate(&lobby, spectator, "watch_99").await;
    let _ = spec_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!("first message must be MatchCreated")
    };
    lobby.join_match(guest, match_id);
    let _ = host_rx.try_recv();
    let _ = guest_rx.try_recv();
    lobby.spectate(spectator, match_id);
    let _ = spec_rx.try_recv(); // SpectateStarted
    let _ = host_rx.try_recv(); // SpectatorJoined
    let _ = guest_rx.try_recv(); // SpectatorJoined

    // X plays a move. All three receive a BoardUpdate.
    lobby.make_move(host, Position::new(0).unwrap());
    assert!(matches!(
        host_rx.try_recv().unwrap(),
        ServerMessage::BoardUpdate { .. }
    ));
    assert!(matches!(
        guest_rx.try_recv().unwrap(),
        ServerMessage::BoardUpdate { .. }
    ));
    assert!(matches!(
        spec_rx.try_recv().unwrap(),
        ServerMessage::BoardUpdate { .. }
    ));
}

#[tokio::test]
async fn spectate_rejects_non_existent_match() {
    let lobby = fast_lobby();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = lobby.register_client(tx);
    authenticate(&lobby, client, "watch_99").await;
    let _ = rx.try_recv();

    lobby.spectate(client, MatchId::new(999));
    match rx.try_recv().unwrap() {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::MatchNotFound),
        other => panic!("expected MatchNotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn spectate_rejects_a_full_match() {
    use common::protocol::MAX_SPECTATORS;

    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    let _ = guest_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!()
    };
    lobby.join_match(guest, match_id);
    let _ = host_rx.try_recv();
    let _ = guest_rx.try_recv();

    // Fill the match with spectators.
    for index in 0..MAX_SPECTATORS {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let spectator = lobby.register_client(tx);
        authenticate(&lobby, spectator, &format!("fan_{index:02}")).await;
        let _ = rx.try_recv();
        lobby.spectate(spectator, match_id);
        // Drain SpectateStarted and the joined notifications.
        let _ = rx.try_recv();
        while host_rx.try_recv().is_ok() {}
        while guest_rx.try_recv().is_ok() {}
    }

    // The next spectator is rejected.
    let (tx, mut rx) = mpsc::unbounded_channel();
    let overflow = lobby.register_client(tx);
    authenticate(&lobby, overflow, "late_fan").await;
    let _ = rx.try_recv();
    lobby.spectate(overflow, match_id);
    match rx.try_recv().unwrap() {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::SpectatorLimitReached),
        other => panic!("expected SpectatorLimitReached, got {other:?}"),
    }
}

#[tokio::test]
async fn spectate_rejects_a_player_of_the_same_match() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!()
    };

    lobby.spectate(host, match_id);
    match host_rx.try_recv().unwrap() {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::InvalidState),
        other => panic!("expected InvalidState, got {other:?}"),
    }
}

#[tokio::test]
async fn spectate_rejects_a_second_match() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (spec_tx, mut spec_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let spectator = lobby.register_client(spec_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, spectator, "watch_99").await;
    let _ = spec_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!()
    };
    lobby.spectate(spectator, match_id);
    let _ = spec_rx.try_recv();
    let _ = host_rx.try_recv();

    lobby.spectate(spectator, match_id);
    match spec_rx.try_recv().unwrap() {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::AlreadySpectating),
        other => panic!("expected AlreadySpectating, got {other:?}"),
    }
}

#[tokio::test]
async fn leave_spectate_notifies_players() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (spec_tx, mut spec_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let spectator = lobby.register_client(spec_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, spectator, "watch_99").await;
    let _ = spec_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!()
    };
    lobby.spectate(spectator, match_id);
    let _ = spec_rx.try_recv();
    let _ = host_rx.try_recv();

    lobby.leave_spectate(spectator);
    match host_rx.try_recv().unwrap() {
        ServerMessage::SpectatorLeft {
            username,
            spectator_count,
        } => {
            assert_eq!(username, "watch_99 name");
            assert_eq!(spectator_count, 0);
        }
        other => panic!("expected SpectatorLeft, got {other:?}"),
    }

    // The session is no longer marked as spectating.
    assert!(!lobby.is_spectating(spectator));
}

#[tokio::test]
async fn match_abandoned_reaches_spectators() {
    let lobby = fast_lobby();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (guest_tx, _guest_rx) = mpsc::unbounded_channel();
    let (spec_tx, mut spec_rx) = mpsc::unbounded_channel();
    let host = lobby.register_client(host_tx);
    let guest = lobby.register_client(guest_tx);
    let spectator = lobby.register_client(spec_tx);
    authenticate(&lobby, host, "host_99").await;
    let _ = host_rx.try_recv();
    authenticate(&lobby, guest, "guest_99").await;
    authenticate(&lobby, spectator, "watch_99").await;
    let _ = spec_rx.try_recv();

    lobby.create_match(host);
    let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
        unreachable!()
    };
    lobby.join_match(guest, match_id);
    lobby.spectate(spectator, match_id);
    let _ = spec_rx.try_recv();
    while host_rx.try_recv().is_ok() {}

    // The guest disconnects; the match is dissolved.
    lobby.disconnect(guest);
    match spec_rx.try_recv().unwrap() {
        ServerMessage::MatchAbandoned { .. } => {}
        other => panic!("expected MatchAbandoned, got {other:?}"),
    }
    assert!(!lobby.is_spectating(spectator));
}
