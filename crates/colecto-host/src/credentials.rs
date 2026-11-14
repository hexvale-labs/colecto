//! Per-extension credentials for client parts (docs/extensions/ui-model.md, "Credentials").
//!
//! The shell mints one token per running extension with the owner bearer and keeps it out of
//! the extension's Worker. Only SHA-256 verifiers live here, in memory: a Host restart forgets
//! every token. A token is bound to the extension's generation, so disabling, removing or
//! reinstalling it invalidates every token it holds at the next call.
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Live tokens per extension (one per open window); minting another drops the oldest.
pub const MAX_PER_EXTENSION: usize = 8;

struct Entry {
    id: String,
    generation: u64,
    minted: u64,
}

#[derive(Default)]
pub struct Credentials {
    entries: HashMap<[u8; 32], Entry>,
    counter: u64,
}

fn verifier(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

impl Credentials {
    pub fn mint(&mut self, id: &str, generation: u64) -> Result<String, String> {
        let token = crate::api_keys::random()?;
        self.counter += 1;
        let mine: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, e)| e.id == id)
            .map(|(k, e)| (e.minted, *k))
            .collect();
        if mine.len() >= MAX_PER_EXTENSION
            && let Some((_, oldest)) = mine.into_iter().min()
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(
            verifier(&token),
            Entry {
                id: id.into(),
                generation,
                minted: self.counter,
            },
        );
        Ok(token)
    }
    /// The extension and generation a token was minted for.
    pub fn resolve(&self, token: &str) -> Option<(String, u64)> {
        self.entries
            .get(&verifier(token))
            .map(|e| (e.id.clone(), e.generation))
    }
    /// Revoke one token, but only on behalf of the extension it belongs to.
    pub fn revoke(&mut self, id: &str, token: &str) {
        let key = verifier(token);
        if self.entries.get(&key).is_some_and(|e| e.id == id) {
            self.entries.remove(&key);
        }
    }
    /// Forget tokens whose extension is gone or moved to another generation.
    pub fn retain(&mut self, current: impl Fn(&str) -> Option<u64>) {
        self.entries
            .retain(|_, e| current(&e.id) == Some(e.generation));
    }
}

/// A syntactically valid token: what `api_keys::random` produces.
pub fn plausible(token: &str) -> bool {
    (32..=128).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_resolve_to_their_extension_and_the_oldest_is_dropped() {
        let mut credentials = Credentials::default();
        let first = credentials.mint("a", 1).unwrap();
        assert!(plausible(&first));
        assert_eq!(credentials.resolve(&first), Some(("a".into(), 1)));
        assert_eq!(credentials.resolve("nope"), None);
        let others: Vec<_> = (0..MAX_PER_EXTENSION)
            .map(|_| credentials.mint("a", 1).unwrap())
            .collect();
        assert_eq!(credentials.resolve(&first), None);
        assert!(others.iter().all(|t| credentials.resolve(t).is_some()));
        // Another extension cannot revoke a token it does not own.
        credentials.revoke("b", &others[0]);
        assert!(credentials.resolve(&others[0]).is_some());
        credentials.revoke("a", &others[0]);
        assert!(credentials.resolve(&others[0]).is_none());
        credentials.retain(|id| (id == "a").then_some(2));
        assert!(others.iter().all(|t| credentials.resolve(t).is_none()));
    }
}
