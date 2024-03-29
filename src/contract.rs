#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, Event, MessageInfo, Response,
    StdResult, SubMsg, Uint128,
};
use cozy_chess::{Board, Color, GameStatus, Move};
use cw2::{ensure_from_older_version, set_contract_version};
use sha2::{Digest, Sha256};
use std::str::FromStr;

use crate::error::{ContractError, InvalidBetReason};
use crate::game::{Match, MatchState, NextMove, MOVE_FEN_LENGTH};
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{
    increment_nonce, MatchId, ADMIN, MATCHES, MATCH_IDS, MIN_BET, NEXT_NONCE, PLAYER_MATCHES,
};

// Version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw-chess";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    ADMIN.save(deps.storage, &info.sender)?;
    MIN_BET.save(deps.storage, &(msg.min_bet.amount, msg.min_bet.denom))?;
    NEXT_NONCE.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", info.sender))
}

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    use ExecuteMsg::*;

    match msg {
        CreateMatch { opponent } => exec::create_match(deps, info, opponent),
        AbortMatch { match_id } => exec::abort_match(deps, info, match_id),
        JoinMatch { match_id } => exec::join_match(deps, env, info, match_id),
        MakeMove { match_id, move_fen } => exec::make_move(deps, env, info, match_id, move_fen),
    }
}

pub(crate) mod exec {
    use super::*;

    pub fn create_match(
        deps: DepsMut,
        info: MessageInfo,
        opponent: Addr,
    ) -> Result<Response, ContractError> {
        let challenger = info.sender;
        let opponent = validate_address(deps.api, opponent.as_str())?;
        validate_players(&challenger, &opponent)?;

        let min_bet = MIN_BET.load(deps.storage)?;
        let bet = validate_bet(&info.funds, &Coin::new(min_bet.0.into(), min_bet.1))?;

        let nonce = NEXT_NONCE.load(deps.storage)?;

        let new_match = Match::new(challenger.clone(), opponent.clone(), nonce, bet);
        let match_id = match_id(&challenger, &opponent, nonce);

        save_match_state(deps.storage, match_id, &new_match)?;
        save_player_match(deps.storage, &challenger, match_id)?;
        save_player_match(deps.storage, &opponent, match_id)?;
        save_match_id(deps.storage, nonce, match_id)?;
        increment_nonce(deps.storage)?;

        Ok(Response::new()
            .add_attribute("action", "create_match")
            .add_attribute("sender", &challenger)
            .add_event(
                Event::new("match_created")
                    .add_attribute("challenger", challenger)
                    .add_attribute("opponent", opponent)
                    .add_attribute("match_id", hex::encode(match_id)), // Convert match_id to hexadecimal string
            ))
    }

    pub fn abort_match(
        deps: DepsMut,
        info: MessageInfo,
        match_id: String,
    ) -> Result<Response, ContractError> {
        let challenger = info.sender;
        let match_id = validate_match_id(&match_id)?;
        let chess_match = lookup_match(&deps, match_id)?;
        validate_match_creator(&chess_match, &challenger)?;
        ensure_awaiting_opponent(&chess_match)?;

        clean_match_state(deps.storage, match_id, &chess_match);

        Ok(Response::new()
            .add_attribute("action", "abort_match")
            .add_attribute("sender", &challenger)
            .add_event(
                Event::new("match_aborted").add_attribute("match_id", hex::encode(match_id)), // Convert match_id to hexadecimal string
            ))
    }

    pub fn join_match(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        match_id: String,
    ) -> Result<Response, ContractError> {
        let opponent = info.sender;

        let match_id = validate_match_id(&match_id)?;
        let mut chess_match = lookup_match(&deps, match_id)?;
        validate_opponent(&opponent, &chess_match.opponent)?;

        let bet = validate_bet(&info.funds, &chess_match.bet)?;
        validate_opponent_bet(&chess_match.bet.amount, &bet.amount)?;
        ensure_awaiting_opponent(&chess_match)?;

        chess_match.start(env.block.height);
        save_match_state(deps.storage, match_id, &chess_match)?;

        Ok(Response::new()
            .add_attribute("action", "join_match")
            .add_attribute("sender", &opponent)
            .add_event(
                Event::new("match_started").add_attribute("match_id", hex::encode(match_id)),
            ))
    }

    pub fn make_move(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        match_id: String,
        move_fen: String,
    ) -> Result<Response, ContractError> {
        let player = info.sender;
        let match_id = validate_match_id(&match_id)?;
        validate_fen_move(&move_fen)?;

        let mut chess_match = lookup_match(&deps, match_id)?;
        validate_match_state(&chess_match, &player)?;

        let mut board = decode_board(&chess_match.board)?;
        let mov = decode_move(&move_fen)?;
        validate_move(&board, &mov)?;

        board.play_unchecked(mov);
        update_match(&mut chess_match, &board, env.block.height);

        let mut msgs: Vec<CosmosMsg> = vec![];
        let mut events = vec![Event::new("move_executed")
            .add_attribute("match_id", hex::encode(match_id))
            .add_attribute("player", &player)
            .add_attribute("move", move_fen)];

        if chess_match.state == MatchState::Won {
            // Match was won with move that was just executed
            events.push(
                Event::new("match_won")
                    .add_attribute("match_id", hex::encode(match_id))
                    .add_attribute("winner", &player)
                    .add_attribute("board", encode_board(&board)),
            );

            // Winner gets both deposits
            // TODO: contract should take a fee (e.g. 1% of the total bet),
            // to be sent to the contract owner (most likely a DAO treasury),
            // and send the rest to the winner.
            transfer_pot_to_winner(&mut msgs, &chess_match, &player);

            // TODO: update elo rating

            // Match is over, clean up storage
            clean_match_state(deps.storage, match_id, &chess_match);
        } else if chess_match.state == MatchState::Drawn {
            // Match drawn, refund deposits to both players
            events.push(
                Event::new("match_drawn")
                    .add_attribute("match_id", hex::encode(match_id))
                    .add_attribute("board", encode_board(&board)),
            );

            refund_players(&mut msgs, &chess_match);

            // TODO: update elo rating

            // Match is over, clean up storage
            clean_match_state(deps.storage, match_id, &chess_match);
        } else {
            // match still ongoing, update on-chain board
            save_match_state(deps.storage, match_id, &chess_match)?;
        }

        let submsgs: Vec<SubMsg<_>> = msgs.into_iter().map(SubMsg::new).collect();
        Ok(Response::new()
            .add_attribute("action", "make_move")
            .add_attribute("sender", &player)
            .add_events(events)
            .add_submessages(submsgs))
    }

    fn clean_match_state(
        storage: &mut dyn cosmwasm_std::Storage,
        match_id: [u8; 32],
        chess_match: &Match,
    ) {
        MATCHES.remove(storage, match_id);
        PLAYER_MATCHES.remove(storage, (&chess_match.challenger, match_id));
        PLAYER_MATCHES.remove(storage, (&chess_match.opponent, match_id));
        MATCH_IDS.remove(storage, chess_match.nonce);
    }

    fn save_match_id(
        storage: &mut dyn cosmwasm_std::Storage,
        nonce: u64,
        match_id: [u8; 32],
    ) -> StdResult<()> {
        MATCH_IDS.save(storage, nonce, &match_id)
    }

    fn save_match_state(
        storage: &mut dyn cosmwasm_std::Storage,
        match_id: [u8; 32],
        chess_match: &Match,
    ) -> StdResult<()> {
        MATCHES.save(storage, match_id, chess_match)
    }

    fn save_player_match(
        storage: &mut dyn cosmwasm_std::Storage,
        player: &Addr,
        match_id: [u8; 32],
    ) -> StdResult<()> {
        PLAYER_MATCHES.save(storage, (player, match_id), &())
    }

    pub(crate) fn match_id(challenger: &Addr, opponent: &Addr, nonce: u64) -> MatchId {
        Sha256::digest(
            [
                challenger.as_bytes(),
                opponent.as_bytes(),
                &nonce.to_be_bytes(),
            ]
            .concat(),
        )
        .into()
    }

    #[allow(dead_code)]
    pub(crate) fn init_board() -> String {
        Board::default().to_string()
    }

    #[inline]
    pub(crate) fn encode_board(board: &Board) -> String {
        board.to_string()
    }

    pub(crate) fn decode_board(board: &str) -> Result<Board, ContractError> {
        match Board::from_str(board) {
            Ok(b) => Ok(b),
            Err(_) => Err(ContractError::InvalidBoardEncoding {}),
        }
    }

    pub(crate) fn decode_move(move_fen: &str) -> Result<Move, ContractError> {
        match Move::from_str(move_fen) {
            Ok(m) => Ok(m),
            Err(_) => Err(ContractError::InvalidMoveEncoding {}),
        }
    }

    fn ensure_awaiting_opponent(chess_match: &Match) -> Result<(), ContractError> {
        if chess_match.state != MatchState::AwaitingOpponent {
            return Err(ContractError::NotAwaitingOpponent {});
        }
        Ok(())
    }

    pub(crate) fn lookup_match(deps: &DepsMut, match_id: [u8; 32]) -> Result<Match, ContractError> {
        match MATCHES.load(deps.storage, match_id) {
            Ok(m) => Ok(m),
            Err(_) => Err(ContractError::UnknownMatch {}),
        }
    }

    fn refund_players(msgs: &mut Vec<CosmosMsg>, chess_match: &Match) {
        msgs.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: chess_match.challenger.to_string(),
            amount: vec![chess_match.bet.clone()],
        }));
        msgs.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: chess_match.opponent.to_string(),
            amount: vec![chess_match.bet.clone()],
        }));
    }

    fn update_match(chess_match: &mut Match, board: &Board, height: u64) {
        chess_match.state = match board.status() {
            GameStatus::Ongoing => match board.side_to_move() {
                Color::White => MatchState::OnGoing(NextMove::Whites),
                Color::Black => MatchState::OnGoing(NextMove::Blacks),
            },
            GameStatus::Won => MatchState::Won,
            GameStatus::Drawn => MatchState::Drawn,
        };
        chess_match.board = encode_board(board);
        chess_match.last_move = height;
    }

    fn transfer_pot_to_winner(msgs: &mut Vec<CosmosMsg>, chess_match: &Match, winner: &Addr) {
        msgs.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: winner.to_string(),
            amount: vec![Coin::new(
                chess_match.bet.amount.u128() * 2,
                &chess_match.bet.denom,
            )],
        }));
    }

    fn validate_address(api: &dyn cosmwasm_std::Api, addr: &str) -> Result<Addr, ContractError> {
        match api.addr_validate(addr) {
            Ok(addr) => Ok(addr),
            Err(_) => Err(ContractError::InvalidAddress {}),
        }
    }

    fn validate_bet(funds: &[cosmwasm_std::Coin], min_bet: &Coin) -> Result<Coin, ContractError> {
        if funds.is_empty() {
            return Err(ContractError::InvalidBet {
                reason: InvalidBetReason::MissingBet,
            });
        }

        if funds.len() > 1 {
            return Err(ContractError::InvalidBet {
                reason: InvalidBetReason::TooManyCoins,
            });
        }

        let bet = &funds[0];
        if bet.denom != min_bet.denom {
            return Err(ContractError::InvalidBet {
                reason: InvalidBetReason::WrongDenom,
            });
        } else if bet.amount.lt(&min_bet.amount) {
            return Err(ContractError::InvalidBet {
                reason: InvalidBetReason::AmountTooLow,
            });
        }

        Ok(bet.clone())
    }

    fn validate_fen_move(move_fen: &str) -> Result<(), ContractError> {
        if move_fen.len() != MOVE_FEN_LENGTH {
            return Err(ContractError::InvalidMoveEncoding {});
        }
        Ok(())
    }

    fn validate_match_state(chess_match: &Match, player: &Addr) -> Result<(), ContractError> {
        match chess_match.state {
            MatchState::AwaitingOpponent => {
                return Err(ContractError::StillAwaitingOpponent {});
            }
            MatchState::Won | MatchState::Drawn => {
                return Err(ContractError::MatchAlreadyFinished {});
            }
            MatchState::OnGoing(NextMove::Whites) => {
                if player != chess_match.challenger {
                    return Err(ContractError::NotYourTurn {});
                }
            }
            MatchState::OnGoing(NextMove::Blacks) => {
                if player != chess_match.opponent {
                    return Err(ContractError::NotYourTurn {});
                }
            }
        }
        Ok(())
    }

    fn validate_move(board: &Board, mov: &Move) -> Result<(), ContractError> {
        if !board.is_legal(*mov) {
            return Err(ContractError::IllegalMove {});
        }
        Ok(())
    }

    fn validate_opponent(expected: &Addr, actual: &Addr) -> Result<(), ContractError> {
        if expected != actual {
            return Err(ContractError::InvalidOpponent {});
        }

        Ok(())
    }

    fn validate_opponent_bet(
        initial_bet: &Uint128,
        opponent_bet: &Uint128,
    ) -> Result<(), ContractError> {
        if opponent_bet != initial_bet {
            return Err(ContractError::InvalidBet {
                reason: InvalidBetReason::InvalidAmount,
            });
        }

        Ok(())
    }

    fn validate_players(challenger: &Addr, opponent: &Addr) -> Result<(), ContractError> {
        if challenger == opponent {
            return Err(ContractError::InvalidOpponent {});
        }

        Ok(())
    }

    fn validate_match_id(match_id: &str) -> Result<MatchId, ContractError> {
        let mut bytes = [0u8; 32];
        let match_id = match hex::decode_to_slice(match_id, &mut bytes) {
            Ok(_) => bytes,
            Err(_) => return Err(ContractError::InvalidMatchId {}),
        };
        Ok(match_id)
    }

    fn validate_match_creator(chess_match: &Match, addr: &Addr) -> Result<(), ContractError> {
        if addr != chess_match.challenger {
            return Err(ContractError::NotMatchCreator {});
        }
        Ok(())
    }
}

pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let _original_version =
        ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}
