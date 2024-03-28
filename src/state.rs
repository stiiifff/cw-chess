use cosmwasm_std::{Addr, StdResult, Storage, Uint128};
use cw_storage_plus::{Item, Map};

use crate::game::Match;

pub type MatchId = [u8; 32];

// Contract admin address
pub const ADMIN: Item<Addr> = Item::new("contract_admin");

// Minigmum bet amount to start a game
pub const MIN_BET: Item<(Uint128, String)> = Item::new("min_bet");

pub const NEXT_NONCE: Item<u64> = Item::new("next_nonce");

pub const MATCHES: Map<MatchId, Match> = Map::new("matches");
pub const MATCH_IDS: Map<u64, MatchId> = Map::new("match_ids");
pub const PLAYER_MATCHES: Map<(Addr, MatchId), ()> = Map::new("player_matches");

pub fn increment_nonce(store: &mut dyn Storage) -> StdResult<u64> {
    let nonce: u64 = NEXT_NONCE.may_load(store)?.unwrap_or_default() + 1;
    NEXT_NONCE.save(store, &nonce)?;
    Ok(nonce)
}
