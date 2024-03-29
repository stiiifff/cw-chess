use cosmwasm_std::{Addr, Coin};
use cozy_chess::{Board, Color, FenParseError, GameStatus, IllegalMoveError, Move, MoveParseError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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
    board: String, // Don't expose this
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

    pub fn new_ext(
        challenger: Addr,
        opponent: Addr,
        state: MatchState,
        nonce: u64,
        last_move: u64,
        start: u64,
        bet: Coin,
    ) -> Match {
        Self {
            challenger,
            opponent,
            board: Board::default().to_string(),
            state,
            nonce,
            // style,
            last_move,
            start,
            bet,
        }
    }

    pub fn board(&self) -> String {
        self.board.to_owned()
    }

    pub fn start(&mut self, block_height: u64) {
        self.state = MatchState::OnGoing(NextMove::Whites);
        self.start = block_height;
    }

    pub fn play_move(&mut self, mov: &Move, block_height: u64) -> Result<&Self, IllegalMoveError> {
        let mut board = Match::decode_board(&self.board)
            .expect("Board encoding should always be correct as it is controlled by Match.");

        board.try_play(*mov)?;

        self.state = match board.status() {
            GameStatus::Ongoing => match board.side_to_move() {
                Color::White => MatchState::OnGoing(NextMove::Whites),
                Color::Black => MatchState::OnGoing(NextMove::Blacks),
            },
            GameStatus::Won => MatchState::Won,
            GameStatus::Drawn => MatchState::Drawn,
        };
        self.board = Match::encode_board(&board);
        self.last_move = block_height;
        Ok(self)
    }

    pub fn decode_board(board: &str) -> Result<Board, FenParseError> {
        Board::from_str(board)
    }

    pub fn decode_move(move_fen: &str) -> Result<Move, MoveParseError> {
        Move::from_str(move_fen)
    }

    pub fn encode_board(board: &Board) -> String {
        board.to_string()
    }

    #[cfg(test)]
    pub(crate) fn set_board_state(mut self, board: String) -> Self {
        self.board = board;
        self
    }
}
