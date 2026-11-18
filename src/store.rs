//! The per-player food history, in the save's own mod data folder.
//!
//! The retired bundle wrote `<world>/solcarrot/foods.tsv` itself. The SDK gives a mod a
//! save-scoped file store instead: `<save>/horizon-mod-data/solcarrot/foods.tsv`, written
//! atomically by the service and copied, backed up and deleted with the save. The bytes are
//! the format [`crate::runtime`] documents, unchanged (`format_store` / `parse_store`).

use std::collections::BTreeMap;

use mod_api::{ConfigPath, ServerHost};

use crate::runtime::{FoodHistory, format_store, parse_store};

/// The file's name inside the save's `horizon-mod-data/solcarrot/` folder.
pub const STORE_FILE: &str = "foods.tsv";

/// The stored histories, by player UUID. A missing, unreadable or malformed file is an
/// empty history, never a failed world open.
pub fn load(host: &dyn ServerHost) -> BTreeMap<u128, FoodHistory> {
    let Ok(path) = ConfigPath::new(STORE_FILE) else {
        return BTreeMap::new();
    };
    match host.world_data_files().read(&path) {
        Ok(bytes) => parse_store(&String::from_utf8_lossy(&bytes)),
        Err(_) => BTreeMap::new(),
    }
}

/// Replace the stored histories. A refused write (no world open any more) loses nothing the
/// previous food's save did not already put on disk.
pub fn save(host: &dyn ServerHost, players: &BTreeMap<u128, FoodHistory>) {
    let Ok(path) = ConfigPath::new(STORE_FILE) else {
        return;
    };
    let _ = host
        .world_data_files()
        .write(&path, format_store(players).as_bytes());
}
