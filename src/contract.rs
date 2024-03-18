#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    Addr, Binary, Deps, DepsMut, Env, Event, MessageInfo, Response, StdError, StdResult,
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
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    use ExecuteMsg::*;

    match msg {
        CreateMatch { opponent } => exec::create_match(deps, info, opponent),
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
        let opponent = match deps.api.addr_validate(&opponent) {
            Ok(addr) => addr,
            Err(_) => return Err(ContractError::InvalidAddress {}),
        };

        if challenger == opponent {
            return Err(ContractError::InvalidOpponent {});
        }

        let min_bet = MIN_BET.load(deps.storage)?;
        let challenger_bet = &info.funds[0];

        if info.funds.len() != 1 {
            return Err(ContractError::InvalidBet {
                reason: InvalidBetReason::TooManyCoins,
            });
        } else if challenger_bet.denom != min_bet.1 {
            return Err(ContractError::InvalidBet {
                reason: InvalidBetReason::WrongDenom,
            });
        } else if challenger_bet.amount.lt(&min_bet.0) {
            return Err(ContractError::InvalidBet {
                reason: InvalidBetReason::AmountTooLow,
            });
        }

        let nonce = NEXT_NONCE.load(deps.storage)?;

        let new_match = Match {
            challenger: challenger.clone(),
            opponent: opponent.clone(),
            board: init_board(),
            state: MatchState::AwaitingOpponent,
            nonce,
            // style,
            last_move: 0u64,
            start: 0u64,
            bet: challenger_bet.clone(),
        };

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

    fn match_id(challenger: Addr, opponent: Addr, nonce: u64) -> MatchId {
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

    pub fn init_board() -> String {
        Board::default().to_string()
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
            (player_b_addr.as_ref(), &[players_balance]),
        ]);

        let msg = InstantiateMsg {
            min_bet: Coin::new(10, NATIVE_DENOM),
        };
        let info = mock_info("admin", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg::CreateMatch {
            opponent: player_b_addr.to_string(),
        };
        let info = mock_info(player_a_addr.as_ref(), &[bet.clone()]);

        // we can just call .unwrap() to assert this was a success
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let match_id = Sha256::digest(
            &[
                player_a_addr.as_bytes(),
                player_b_addr.as_bytes(),
                &0u64.to_be_bytes(),
            ]
            .concat(),
        )
        .into();

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

        // let player_a_balance = deps
        //     .querier.
        //     .query_balance(player_a_addr.clone(), NATIVE_DENOM)
        //     .unwrap();
        // assert_eq!(player_a_balance, players_balance);

        // let contract_balance = deps
        //     .querier
        //     .query_balance(Addr::unchecked("contract"), NATIVE_DENOM)
        //     .unwrap();
        // assert_eq!(contract_balance, Coin::new(10, NATIVE_DENOM));

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
}
