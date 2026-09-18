//! Top-players ranking.
//!
//! [`RankingService`] tracks the number of wins per registered user. It is
//! updated when a match ends with a real victory (not by abandonment, and
//! not by a draw). The service is intentionally independent from the lobby:
//! it only knows about `Username` and the win counter, which keeps the
//! dependency graph acyclic and the module trivially testable.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use common::domain::{RankingEntry, Username};

/// Number of entries returned by [`RankingService::top`] when the caller
/// asks for the default "top players" list.
pub const TOP_N: usize = 10;

/// Tracks the number of wins per registered user.
pub struct RankingService {
    entries: Mutex<HashMap<Username, RankingEntry>>,
}

impl Default for RankingService {
    fn default() -> Self {
        Self::new()
    }
}

impl RankingService {
    /// Creates an empty ranking.
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Records a win for the given user.
    ///
    /// If the user already has an entry, its `wins` counter is incremented
    /// and its `name` is updated to the most recent value. Otherwise a new
    /// entry is created with `wins = 1`.
    pub fn record_win(&self, username: &Username, name: &str) {
        let mut entries = self.lock();
        entries
            .entry(username.clone())
            .and_modify(|entry| {
                entry.wins = entry.wins.saturating_add(1);
                entry.name = name.to_string();
            })
            .or_insert_with(|| RankingEntry {
                username: username.clone(),
                name: name.to_string(),
                wins: 1,
            });
    }

    /// Returns up to `limit` top entries.
    ///
    /// Entries are ordered by `wins` descending. Ties are broken by
    /// `username` ascending so the order is deterministic and stable across
    /// calls.
    pub fn top(&self, limit: usize) -> Vec<RankingEntry> {
        let entries = self.lock();
        let mut sorted: Vec<RankingEntry> = entries.values().cloned().collect();
        sorted.sort_by(|a, b| {
            b.wins
                .cmp(&a.wins)
                .then_with(|| a.username.key().cmp(b.username.key()))
        });
        sorted.truncate(limit);
        sorted
    }

    /// Returns the number of users with at least one win.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Returns `true` when no user has won a game yet.
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Username, RankingEntry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn username(value: &str) -> Username {
        Username::new(value).unwrap()
    }

    #[test]
    fn new_ranking_is_empty() {
        let ranking = RankingService::new();
        assert!(ranking.is_empty());
        assert_eq!(ranking.len(), 0);
        assert!(ranking.top(TOP_N).is_empty());
    }

    #[test]
    fn record_win_creates_a_new_entry() {
        let ranking = RankingService::new();
        ranking.record_win(&username("alice_99"), "Alice");
        assert_eq!(ranking.len(), 1);
        let top = ranking.top(TOP_N);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].username.as_str(), "alice_99");
        assert_eq!(top[0].name, "Alice");
        assert_eq!(top[0].wins, 1);
    }

    #[test]
    fn record_win_increments_the_counter() {
        let ranking = RankingService::new();
        let user = username("alice_99");
        ranking.record_win(&user, "Alice");
        ranking.record_win(&user, "Alice");
        ranking.record_win(&user, "Alice");
        assert_eq!(ranking.len(), 1);
        assert_eq!(ranking.top(TOP_N)[0].wins, 3);
    }

    #[test]
    fn record_win_updates_the_display_name() {
        let ranking = RankingService::new();
        let user = username("alice_99");
        ranking.record_win(&user, "Alice");
        ranking.record_win(&user, "Alice Renamed");
        let top = ranking.top(TOP_N);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].name, "Alice Renamed");
    }

    #[test]
    fn record_win_is_case_insensitive_on_username() {
        let ranking = RankingService::new();
        ranking.record_win(&username("alice_99"), "Alice");
        ranking.record_win(&username("ALICE_99"), "Alice");
        assert_eq!(ranking.len(), 1);
        assert_eq!(ranking.top(TOP_N)[0].wins, 2);
    }

    #[test]
    fn top_orders_by_wins_descending() {
        let ranking = RankingService::new();
        ranking.record_win(&username("alice_99"), "Alice");
        ranking.record_win(&username("alice_99"), "Alice");
        ranking.record_win(&username("alice_99"), "Alice");
        ranking.record_win(&username("bob_77"), "Bob");
        ranking.record_win(&username("carol_55"), "Carol");
        ranking.record_win(&username("carol_55"), "Carol");

        let top = ranking.top(TOP_N);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].username.as_str(), "alice_99");
        assert_eq!(top[1].username.as_str(), "carol_55");
        assert_eq!(top[2].username.as_str(), "bob_77");
    }

    #[test]
    fn top_breaks_ties_by_username_ascending() {
        let ranking = RankingService::new();
        ranking.record_win(&username("carol_55"), "Carol");
        ranking.record_win(&username("alice_99"), "Alice");
        ranking.record_win(&username("bob_77"), "Bob");

        let top = ranking.top(TOP_N);
        assert_eq!(top[0].username.as_str(), "alice_99");
        assert_eq!(top[1].username.as_str(), "bob_77");
        assert_eq!(top[2].username.as_str(), "carol_55");
    }

    #[test]
    fn top_respects_the_limit() {
        let ranking = RankingService::new();
        for index in 0..15 {
            let name = format!("user_{index:02}");
            ranking.record_win(&username(&name), "Name");
        }
        assert_eq!(ranking.top(TOP_N).len(), TOP_N);
        assert_eq!(ranking.top(3).len(), 3);
        assert_eq!(ranking.top(0).len(), 0);
    }

    #[test]
    fn top_truncates_before_the_limit_when_there_are_fewer_entries() {
        let ranking = RankingService::new();
        ranking.record_win(&username("alice_99"), "Alice");
        assert_eq!(ranking.top(TOP_N).len(), 1);
    }
}
