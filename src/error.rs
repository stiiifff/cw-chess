use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid address")]
    InvalidAddress {},

    #[error("Invalid bet")]
    InvalidBet { reason: InvalidBetReason },

    #[error("Invalid opponent")]
    InvalidOpponent {},

    #[error("Invalid match ID")]
    InvalidMatchId {},

    #[error("Unknown match")]
    UnknownMatch {},

    #[error("Not the match creator")]
    NotMatchCreator {},

    #[error("Not awaiting opponent")]
    NotAwaitingOpponent {},

    #[error("Invalid move encoding")]
    InvalidMoveEncoding {},

    #[error("Still awaiting opponent")]
    StillAwaitingOpponent {},

    #[error("Match already finished")]
    MatchAlreadyFinished {},

    #[error("Not your turn")]
    NotYourTurn {},

    #[error("Illegal move")]
    IllegalMove {},
}

#[derive(Error, Debug, PartialEq)]
pub enum InvalidBetReason {
    #[error("Wrong denomination")]
    WrongDenom,
    #[error("Amount too low")]
    AmountTooLow,
    #[error("Missing bet")]
    MissingBet,
    #[error("Too many coins sent")]
    TooManyCoins,
    #[error("Invalid amount")]
    InvalidAmount,
}
