//! User registry and password hashing.
//!
//! [`AuthService`] owns the in-memory map of registered users and exposes
//! two operations:
//!
//! - [`AuthService::register`]: inserts a new user, hashing the password
//!   with Argon2id. The caller is responsible for validating the raw input
//!   before calling this method.
//! - [`AuthService::authenticate`]: looks up the user and verifies the
//!   password against the stored hash. The two failure modes (user does not
//!   exist, wrong password) are intentionally indistinguishable to avoid
//!   user enumeration.
//!
//! Password hashing is CPU-bound and runs on a dedicated blocking thread
//! through [`tokio::task::spawn_blocking`], so the async runtime is never
//! stalled. The plaintext password is held in a `Vec<u8>` that is explicitly
//! zeroed with `zeroize` once the hash or the verification completes.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use common::domain::{UserProfile, Username};
use common::protocol::AuthFailureReason;
use rand_core::OsRng;
use thiserror::Error;
use zeroize::Zeroize;

use crate::domain::UserRecord;

/// Internal errors raised by the auth service.
///
/// These are not user-facing. They only occur when the underlying blocking
/// task panics or is cancelled, which is a bug, not a user mistake.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Password hashing failed.
    #[error("password hashing failed")]
    Hashing,
    /// The blocking hashing task panicked or was cancelled.
    #[error("blocking hashing task failed")]
    TaskFailed,
}

/// Registers users and verifies passwords.
pub struct AuthService {
    users: Mutex<HashMap<Username, UserRecord>>,
    params: Params,
}

impl Default for AuthService {
    fn default() -> Self {
        Self::with_params(Params::default())
    }
}

impl AuthService {
    /// Creates a service with the given Argon2 parameters.
    ///
    /// Production code uses [`AuthService::default`], which applies the
    /// Argon2 default parameters (19 MiB of memory, 2 iterations). Tests
    /// can supply cheaper parameters to keep the suite fast without
    /// changing the code paths under test.
    pub fn with_params(params: Params) -> Self {
        Self {
            users: Mutex::new(HashMap::new()),
            params,
        }
    }

    /// Registers a new user, hashing the password before storing it.
    ///
    /// The caller is expected to have already validated the profile. This
    /// method only checks that the username is not already taken.
    ///
    /// # Errors
    ///
    /// - [`AuthFailureReason::UsernameTaken`] when the username already
    ///   exists. The comparison is case-insensitive.
    /// - [`AuthFailureReason::InternalError`] when hashing fails. This can
    ///   only happen if the blocking task panics, which is a bug.
    pub async fn register(
        &self,
        profile: UserProfile,
        password: String,
    ) -> Result<(), AuthFailureReason> {
        let password_hash =
            hash_password(password, self.params.clone())
                .await
                .map_err(|error| {
                    tracing::error!(%error, "password hashing failed");
                    AuthFailureReason::InternalError
                })?;

        let mut users = self.lock();
        if users.contains_key(&profile.username) {
            return Err(AuthFailureReason::UsernameTaken);
        }
        users.insert(
            profile.username.clone(),
            UserRecord {
                profile,
                password_hash,
            },
        );
        drop(users);
        Ok(())
    }

    /// Authenticates an existing user.
    ///
    /// # Errors
    ///
    /// - [`AuthFailureReason::InvalidCredentials`] when the username does
    ///   not exist or the password does not match. Both cases are
    ///   indistinguishable by design.
    /// - [`AuthFailureReason::InternalError`] when verification fails for
    ///   an infrastructure reason.
    pub async fn authenticate(
        &self,
        username: &Username,
        password: String,
    ) -> Result<UserProfile, AuthFailureReason> {
        let (stored_hash, profile) = {
            let users = self.lock();
            match users.get(username) {
                Some(record) => (record.password_hash.clone(), record.profile.clone()),
                None => return Err(AuthFailureReason::InvalidCredentials),
            }
        };

        let matched = verify_password(password, stored_hash, self.params.clone())
            .await
            .map_err(|error| {
                tracing::error!(%error, "password verification failed");
                AuthFailureReason::InternalError
            })?;

        if matched {
            Ok(profile)
        } else {
            Err(AuthFailureReason::InvalidCredentials)
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Username, UserRecord>> {
        self.users.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

async fn hash_password(password: String, params: Params) -> Result<String, AuthError> {
    tokio::task::spawn_blocking(move || {
        let mut bytes = password.into_bytes();
        let result = {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
            argon2
                .hash_password(&bytes, &salt)
                .map(|hash| hash.to_string())
                .map_err(|_| AuthError::Hashing)
        };
        bytes.zeroize();
        result
    })
    .await
    .map_err(|_| AuthError::TaskFailed)?
}

async fn verify_password(
    password: String,
    stored_hash: String,
    params: Params,
) -> Result<bool, AuthError> {
    tokio::task::spawn_blocking(move || {
        let mut bytes = password.into_bytes();
        let result = {
            let parsed = PasswordHash::new(&stored_hash).map_err(|_| AuthError::Hashing)?;
            let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
            Ok(argon2.verify_password(&bytes, &parsed).is_ok())
        };
        bytes.zeroize();
        result
    })
    .await
    .map_err(|_| AuthError::TaskFailed)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::domain::Age;

    /// Argon2 parameters tuned for test speed.
    ///
    /// The values are the minimum accepted by the crate: 8 KiB of memory
    /// (the spec requires at least `8 * parallelism`), 1 iteration, and
    /// parallelism of 1. This hashes in microseconds while still exercising
    /// the same code path as production.
    fn fast_params() -> Params {
        Params::new(8, 1, 1, None).expect("test parameters are within the accepted range")
    }

    fn fast_service() -> AuthService {
        AuthService::with_params(fast_params())
    }

    fn profile(name: &str, username: &str, age: u8) -> UserProfile {
        UserProfile {
            name: name.to_string(),
            username: Username::new(username).unwrap(),
            age: Age::new(age).unwrap(),
        }
    }

    #[tokio::test]
    async fn register_then_authenticate_succeeds() {
        let service = fast_service();
        service
            .register(
                profile("Alice", "alice_99", 30),
                "hunter2hunter2".to_string(),
            )
            .await
            .unwrap();
        let authenticated = service
            .authenticate(
                &Username::new("alice_99").unwrap(),
                "hunter2hunter2".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(authenticated.name, "Alice");
    }

    #[tokio::test]
    async fn register_rejects_duplicate_username() {
        let service = fast_service();
        service
            .register(
                profile("Alice", "alice_99", 30),
                "hunter2hunter2".to_string(),
            )
            .await
            .unwrap();
        let error = service
            .register(
                profile("Alice", "alice_99", 30),
                "otherpassword".to_string(),
            )
            .await
            .unwrap_err();
        assert_eq!(error, AuthFailureReason::UsernameTaken);
    }

    #[tokio::test]
    async fn register_is_case_insensitive_on_username() {
        let service = fast_service();
        service
            .register(
                profile("Alice", "alice_99", 30),
                "hunter2hunter2".to_string(),
            )
            .await
            .unwrap();
        let error = service
            .register(
                profile("Alice", "ALICE_99", 30),
                "otherpassword".to_string(),
            )
            .await
            .unwrap_err();
        assert_eq!(error, AuthFailureReason::UsernameTaken);
    }

    #[tokio::test]
    async fn authenticate_rejects_wrong_password() {
        let service = fast_service();
        service
            .register(
                profile("Alice", "alice_99", 30),
                "hunter2hunter2".to_string(),
            )
            .await
            .unwrap();
        let error = service
            .authenticate(
                &Username::new("alice_99").unwrap(),
                "wrongpassword".to_string(),
            )
            .await
            .unwrap_err();
        assert_eq!(error, AuthFailureReason::InvalidCredentials);
    }

    #[tokio::test]
    async fn authenticate_rejects_unknown_user() {
        let service = fast_service();
        let error = service
            .authenticate(
                &Username::new("nobody_here").unwrap(),
                "hunter2hunter2".to_string(),
            )
            .await
            .unwrap_err();
        assert_eq!(error, AuthFailureReason::InvalidCredentials);
    }

    #[tokio::test]
    async fn authenticate_accepts_case_insensitive_username() {
        let service = fast_service();
        service
            .register(
                profile("Alice", "alice_99", 30),
                "hunter2hunter2".to_string(),
            )
            .await
            .unwrap();
        service
            .authenticate(
                &Username::new("ALICE_99").unwrap(),
                "hunter2hunter2".to_string(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn hashes_are_unique_across_registrations() {
        let service = fast_service();
        service
            .register(
                profile("Alice", "alice_99", 30),
                "hunter2hunter2".to_string(),
            )
            .await
            .unwrap();
        service
            .register(profile("Bob", "bob_77", 25), "hunter2hunter2".to_string())
            .await
            .unwrap();
        let users = service.lock();
        let alice = users.get(&Username::new("alice_99").unwrap()).unwrap();
        let bob = users.get(&Username::new("bob_77").unwrap()).unwrap();
        let alice_hash = alice.password_hash.clone();
        let bob_hash = bob.password_hash.clone();
        drop(users);
        assert_ne!(alice_hash, bob_hash);
    }
}
