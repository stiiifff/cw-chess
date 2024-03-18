use cosmwasm_std::{Addr, Coin};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Not yet implemented
// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
// pub enum MatchStyle {
//     Bullet, // 1 minute
//     Blitz,  // 5 minutes
//     Rapid,  // 15 minutes
//     Daily,  // 1 day
// }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum NextMove {
    Whites,
    Blacks,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum MatchState {
    AwaitingOpponent,
    OnGoing(NextMove),
    Won,
    Drawn,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Match {
    pub challenger: Addr,
    pub opponent: Addr,
    pub board: String,
    pub state: MatchState,
    pub nonce: u64,
    // Not yet implemented
    // pub style: MatchStyle,
    pub last_move: u64,
    pub start: u64,
    pub bet: Coin,
}

impl Match {}
