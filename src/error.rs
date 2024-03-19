use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
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
}

#[derive(Error, Debug)]
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
