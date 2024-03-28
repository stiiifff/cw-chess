use cosmwasm_std::{Addr, Coin};
use cozy_chess::Board;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MOVE_FEN_LENGTH: usize = 4;

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

impl Match {
    pub fn new(challenger: Addr, opponent: Addr, nonce: u64, bet: Coin) -> Match {
        Self {
            challenger,
            opponent,
            board: Board::default().to_string(),
            state: MatchState::AwaitingOpponent,
            nonce,
            // style,
            last_move: 0u64,
            start: 0u64,
            bet,
        }
    }

    pub fn start(&mut self, block_height: u64) {
        self.state = MatchState::OnGoing(NextMove::Whites);
        self.start = block_height;
    }
}
