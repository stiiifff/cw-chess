#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    Addr, Binary, Coin, Deps, DepsMut, Env, Event, MessageInfo, Response, StdError, StdResult,
    Uint128,
};
use cozy_chess::Board;
use cw2::{ensure_from_older_version, set_contract_version};
use sha2::{Digest, Sha256};

use crate::error::{ContractError, InvalidBetReason};
use crate::game::{Match, MatchState};
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{increment_nonce, MatchId, ADMIN, MATCHES, MIN_BET, NEXT_NONCE, PLAYER_MATCHES};

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
    }
}

mod exec {
    use crate::state::MATCH_IDS;

    use super::*;

    pub fn create_match(
        deps: DepsMut,
        info: MessageInfo,
        opponent: String,
    ) -> Result<Response, ContractError> {
        let challenger = info.sender;
        let opponent = validate_address(&deps, &opponent)?;
        validate_players(&challenger, &opponent)?;

        let min_bet = MIN_BET.load(deps.storage)?;
        let bet = validate_bet(&info.funds, &Coin::new(min_bet.0.into(), min_bet.1))?;

        let nonce = NEXT_NONCE.load(deps.storage)?;

        let new_match = Match::new(challenger.clone(), opponent.clone(), nonce, bet);
        let match_id = match_id(challenger.clone(), opponent.clone(), nonce);

        MATCHES.save(deps.storage, match_id, &new_match)?;
        PLAYER_MATCHES.update(deps.storage, challenger.clone(), |matches| {
            let mut matches = matches.unwrap_or_default();
            matches.push(match_id);
            Result::<Vec<MatchId>, StdError>::Ok(matches)
        })?;
        MATCH_IDS.save(deps.storage, nonce, &match_id)?;

        increment_nonce(deps.storage)?;

        Ok(Response::new()
            .add_attribute("action", "create_match")
            .add_attribute("sender", challenger.clone())
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
        validate_match_state(&chess_match, MatchState::AwaitingOpponent)?;

        MATCHES.remove(deps.storage, match_id);
        PLAYER_MATCHES.remove(deps.storage, challenger.clone());
        MATCH_IDS.remove(deps.storage, chess_match.nonce);

        Ok(Response::new()
            .add_attribute("action", "abort_match")
            .add_attribute("sender", challenger.clone())
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
        let opponent_bet = validate_bet(&info.funds, &chess_match.bet)?;
        validate_opponent_bet(&chess_match.bet.amount, &opponent_bet.amount)?;
        validate_match_state(&chess_match, MatchState::AwaitingOpponent)?;

        chess_match.start(env.block.height);
        MATCHES.save(deps.storage, match_id, &chess_match)?;

        Ok(Response::new()
            .add_attribute("action", "join_match")
            .add_attribute("sender", opponent.clone())
            .add_event(
                Event::new("match_started").add_attribute("match_id", hex::encode(match_id)),
            ))
    }

    pub(crate) fn match_id(challenger: Addr, opponent: Addr, nonce: u64) -> MatchId {
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

    pub(crate) fn lookup_match(deps: &DepsMut, match_id: [u8; 32]) -> Result<Match, ContractError> {
        match MATCHES.load(deps.storage, match_id) {
            Ok(m) => Ok(m),
            Err(_) => Err(ContractError::UnknownMatch {}),
        }
    }

    fn validate_address(deps: &DepsMut, addr: &str) -> Result<Addr, ContractError> {
        match deps.api.addr_validate(addr) {
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
        if initial_bet != opponent_bet {
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

    fn validate_match_state(chess_match: &Match, state: MatchState) -> Result<(), ContractError> {
        if chess_match.state != state {
            return Err(ContractError::NotAwaitingOpponent {});
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

#[cfg(test)]
mod tests {
    use crate::state::MATCH_IDS;

    use super::exec::init_board;
    use super::*;
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_dependencies_with_balances, mock_env, mock_info},
        Addr, Coin, Uint128,
    };
    // use cosmwasm_std::{BalanceResponse, BankQuery, QueryRequest};

    const NATIVE_DENOM: &str = "untrn";

    #[test]
    fn instantiate_succeeds() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            min_bet: Coin::new(10, NATIVE_DENOM),
        };
        let info = mock_info("admin", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
        assert_eq!(0, res.messages.len());

        let admin = ADMIN.load(deps.as_ref().storage).unwrap();
        assert_eq!(info.sender, admin);

        let min_bet = MIN_BET.load(deps.as_ref().storage).unwrap();
        assert_eq!((msg.min_bet.amount, msg.min_bet.denom), min_bet);

        let nonce = NEXT_NONCE.load(deps.as_ref().storage).unwrap();
        assert_eq!(0, nonce);

        let expected = Response::new()
            .add_attribute("action", "instantiate")
            .add_attribute("owner", info.sender);
        assert_eq!(expected, res);
    }

    #[test]
    fn create_match_succeeds() {
        let player_a_addr = Addr::unchecked("neutron1m9l358xunhhwds0568za49mzhvuxx9ux8xafx2");
        let player_b_addr = Addr::unchecked("neutron10h9stc5v6ntgeygf5xf945njqq5h32r54rf7kf");
        let admin_balance = Coin {
            denom: NATIVE_DENOM.to_string(),
            amount: Uint128::new(1000),
        };
        let players_balance = Coin {
            denom: NATIVE_DENOM.to_string(),
            amount: Uint128::new(100),
        };
        let bet = Coin {
            denom: NATIVE_DENOM.to_string(),
            amount: Uint128::new(10),
        };

        let mut deps = mock_dependencies_with_balances(&[
            (Addr::unchecked("admin").as_ref(), &[admin_balance]),
            (player_a_addr.as_ref(), &[players_balance.clone()]),
            (player_b_addr.as_ref(), &[players_balance.clone()]),
        ]);

        let init_msg = InstantiateMsg {
            min_bet: Coin::new(10, NATIVE_DENOM),
        };
        let info = mock_info("admin", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, init_msg).unwrap();

        let create_msg = ExecuteMsg::CreateMatch {
            opponent: player_b_addr.to_string(),
        };
        let info = mock_info(player_a_addr.as_ref(), &[bet.clone()]);

        // we can just call .unwrap() to assert this was a success
        let res = execute(deps.as_mut(), mock_env(), info, create_msg).unwrap();
        assert_eq!(0, res.messages.len());

        // let match_id = Sha256::digest(
        //     &[
        //         player_a_addr.as_bytes(),
        //         player_b_addr.as_bytes(),
        //         &0u64.to_be_bytes(),
        //     ]
        //     .concat(),
        // )
        // .into();
        let match_id = exec::match_id(player_a_addr.clone(), player_b_addr.clone(), 0u64);

        let actual = MATCHES.load(deps.as_ref().storage, match_id).unwrap();
        let expected = Match {
            challenger: player_a_addr.clone(),
            opponent: player_b_addr.clone(),
            board: init_board(),
            state: MatchState::AwaitingOpponent,
            nonce: 0u64,
            last_move: 0u64,
            start: 0u64,
            bet: bet.clone(),
        };
        assert_eq!(expected, actual);

        let player_a_matches = PLAYER_MATCHES
            .load(deps.as_ref().storage, player_a_addr.clone())
            .unwrap();
        assert_eq!(vec![match_id], player_a_matches);

        let match_idx: u64 = 0;
        let stored_id = MATCH_IDS.load(deps.as_ref().storage, match_idx).unwrap();
        assert_eq!(match_id, stored_id);

        let nonce = NEXT_NONCE.load(deps.as_ref().storage).unwrap();
        assert_eq!(1, nonce);

        // Note: that doesn't seem to work .. the player's balance is not updated (maybe use cw_multi_test?)
        // let player_a_balance = query_balance_native(&deps.querier, &player_a_addr);
        // assert_eq!(players_balance.amount - bet.amount, player_a_balance.amount);

        let expected = Response::new()
            .add_attribute("action", "create_match")
            .add_attribute("sender", player_a_addr.clone())
            .add_event(
                Event::new("match_created")
                    .add_attribute("challenger", player_a_addr)
                    .add_attribute("opponent", player_b_addr)
                    .add_attribute("match_id", hex::encode(match_id)), // Convert match_id to hexadecimal string
            );
        assert_eq!(expected, res);
    }

    #[test]
    fn abort_match_succeeds() {
        let player_a_addr = Addr::unchecked("neutron1m9l358xunhhwds0568za49mzhvuxx9ux8xafx2");
        let player_b_addr = Addr::unchecked("neutron10h9stc5v6ntgeygf5xf945njqq5h32r54rf7kf");
        let admin_balance = Coin {
            denom: NATIVE_DENOM.to_string(),
            amount: Uint128::new(1000),
        };
        let players_balance = Coin {
            denom: NATIVE_DENOM.to_string(),
            amount: Uint128::new(100),
        };
        let bet = Coin {
            denom: NATIVE_DENOM.to_string(),
            amount: Uint128::new(10),
        };

        let mut deps = mock_dependencies_with_balances(&[
            (Addr::unchecked("admin").as_ref(), &[admin_balance]),
            (player_a_addr.as_ref(), &[players_balance.clone()]),
            (player_b_addr.as_ref(), &[players_balance.clone()]),
        ]);

        let init_msg = InstantiateMsg {
            min_bet: Coin::new(10, NATIVE_DENOM),
        };
        let info = mock_info("admin", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, init_msg).unwrap();

        let create_msg = ExecuteMsg::CreateMatch {
            opponent: player_b_addr.to_string(),
        };
        let info = mock_info(player_a_addr.as_ref(), &[bet.clone()]);

        // we can just call .unwrap() to assert this was a success
        let res = execute(deps.as_mut(), mock_env(), info, create_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let match_id = exec::match_id(player_a_addr.clone(), player_b_addr.clone(), 0u64);

        let abort_msg = ExecuteMsg::AbortMatch {
            match_id: hex::encode(match_id),
        };
        let info = mock_info(player_a_addr.as_ref(), &[]);
        let res = execute(deps.as_mut(), mock_env(), info, abort_msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(Ok(None), MATCHES.may_load(deps.as_ref().storage, match_id));
        assert_eq!(
            Ok(None),
            PLAYER_MATCHES.may_load(deps.as_ref().storage, player_a_addr.clone())
        );
        assert_eq!(Ok(None), MATCH_IDS.may_load(deps.as_ref().storage, 0u64));

        let expected = Response::new()
            .add_attribute("action", "abort_match")
            .add_attribute("sender", player_a_addr.clone())
            .add_event(
                Event::new("match_aborted").add_attribute("match_id", hex::encode(match_id)), // Convert match_id to hexadecimal string
            );
        assert_eq!(expected, res);
    }

    // fn query_balance_native(querier: &MockQuerier, address: &Addr) -> Coin {
    //     let req: QueryRequest<BankQuery> = QueryRequest::Bank(BankQuery::Balance {
    //         address: address.to_string(),
    //         denom: NATIVE_DENOM.to_string(),
    //     });
    //     let res = querier
    //         .raw_query(&to_json_binary(&req).unwrap())
    //         .unwrap()
    //         .unwrap();
    //     let balance: BalanceResponse = from_json(&res).unwrap();

    //     return balance.amount;
    // }
}
