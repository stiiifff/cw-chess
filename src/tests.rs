use crate::{
    contract::*,
    game::{Match, MatchState, NextMove},
    msg::*,
    state::*,
    ContractError,
};

use cosmwasm_std::{
    testing::{
        mock_dependencies_with_balances, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
    },
    Addr, BankMsg, Coin, CosmosMsg, Env, Event, MessageInfo, OwnedDeps, Response, Uint128,
};
// use cosmwasm_std::{BalanceResponse, BankQuery, QueryRequest};

const NATIVE_DENOM: &str = "untrn";

struct TestContext {
    pub player_a_addr: Addr,
    pub player_b_addr: Addr,
    #[allow(dead_code)]
    pub admin_balance: Coin,
    #[allow(dead_code)]
    pub players_balance: Coin,
    pub bet: Coin,
    pub deps: OwnedDeps<MockStorage, MockApi, MockQuerier>,
    pub env: Env,
}

impl Default for TestContext {
    fn default() -> Self {
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

        let deps = mock_dependencies_with_balances(&[
            (Addr::unchecked("admin").as_ref(), &[admin_balance.clone()]),
            (player_a_addr.as_ref(), &[players_balance.clone()]),
            (player_b_addr.as_ref(), &[players_balance.clone()]),
        ]);

        let env = mock_env();

        TestContext {
            player_a_addr,
            player_b_addr,
            admin_balance,
            players_balance,
            bet,
            deps,
            env,
        }
    }
}

impl TestContext {
    fn new() -> Self {
        TestContext::default()
    }

    fn admin_info(&mut self) -> MessageInfo {
        mock_info("admin", &[])
    }

    fn player_a_info_with_bet(&mut self) -> MessageInfo {
        mock_info(self.player_a_addr.as_ref(), &[self.bet.clone()])
    }

    fn player_b_info_with_bet(&mut self) -> MessageInfo {
        mock_info(self.player_b_addr.as_ref(), &[self.bet.clone()])
    }

    fn player_a_no_bet(&mut self) -> MessageInfo {
        mock_info(self.player_a_addr.as_ref(), &[])
    }

    fn player_b_no_bet(&mut self) -> MessageInfo {
        mock_info(self.player_b_addr.as_ref(), &[])
    }
}

#[test]
fn instantiate_succeeds() {
    let mut ctx = TestContext::new();

    let info = ctx.admin_info();
    let msg = InstantiateMsg {
        min_bet: Coin::new(10, NATIVE_DENOM),
    };
    let res = instantiate(ctx.deps.as_mut(), ctx.env, info.clone(), msg.clone()).unwrap();
    assert_eq!(0, res.messages.len());

    let admin = ADMIN.load(ctx.deps.as_ref().storage).unwrap();
    assert_eq!(info.sender, admin);

    let min_bet = MIN_BET.load(ctx.deps.as_ref().storage).unwrap();
    assert_eq!((msg.min_bet.amount, msg.min_bet.denom), min_bet);

    let nonce = NEXT_NONCE.load(ctx.deps.as_ref().storage).unwrap();
    assert_eq!(0, nonce);

    let expected = Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", info.sender);
    assert_eq!(expected, res);
}

#[test]
fn create_match_succeeds() {
    let mut ctx = TestContext::new();

    let admin_info = ctx.admin_info();
    let init_msg = InstantiateMsg {
        min_bet: Coin::new(10, NATIVE_DENOM),
    };
    let _res = instantiate(ctx.deps.as_mut(), ctx.env.clone(), admin_info, init_msg).unwrap();

    let create_msg = ExecuteMsg::CreateMatch {
        opponent: ctx.player_b_addr.clone(),
    };

    let player_a_info = ctx.player_a_info_with_bet();
    let res = execute(ctx.deps.as_mut(), ctx.env, player_a_info, create_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let match_id = exec::match_id(&ctx.player_a_addr, &ctx.player_b_addr, 0u64);

    let actual = MATCHES.load(ctx.deps.as_ref().storage, match_id).unwrap();
    let expected = Match::new_ext(
        ctx.player_a_addr.clone(),
        ctx.player_b_addr.clone(),
        MatchState::AwaitingOpponent,
        0u64,
        0u64,
        0u64,
        ctx.bet.clone(),
    );
    assert_eq!(expected, actual);

    assert_eq!(
        true,
        PLAYER_MATCHES.has(ctx.deps.as_ref().storage, (&ctx.player_a_addr, match_id))
    );

    assert_eq!(
        true,
        PLAYER_MATCHES.has(ctx.deps.as_ref().storage, (&ctx.player_b_addr, match_id))
    );

    let match_idx: u64 = 0;
    let stored_id = MATCH_IDS
        .load(ctx.deps.as_ref().storage, match_idx)
        .unwrap();
    assert_eq!(match_id, stored_id);

    let nonce = NEXT_NONCE.load(ctx.deps.as_ref().storage).unwrap();
    assert_eq!(1, nonce);

    // Note: that doesn't seem to work .. the player's balance is not updated (maybe use cw_multi_test?)
    // let player_a_balance = query_balance_native(&deps.querier, &player_a_addr);
    // assert_eq!(players_balance.amount - bet.amount, player_a_balance.amount);

    let expected = Response::new()
        .add_attribute("action", "create_match")
        .add_attribute("sender", &ctx.player_a_addr)
        .add_event(
            Event::new("match_created")
                .add_attribute("challenger", ctx.player_a_addr)
                .add_attribute("opponent", ctx.player_b_addr)
                .add_attribute("match_id", hex::encode(match_id)), // Convert match_id to hexadecimal string
        );
    assert_eq!(expected, res);
}

#[test]
fn abort_match_succeeds() {
    let mut ctx = TestContext::new();

    let admin_info = ctx.admin_info();
    let init_msg = InstantiateMsg {
        min_bet: Coin::new(10, NATIVE_DENOM),
    };
    let _res = instantiate(ctx.deps.as_mut(), ctx.env.clone(), admin_info, init_msg).unwrap();

    let create_msg = ExecuteMsg::CreateMatch {
        opponent: ctx.player_b_addr.clone(),
    };

    let player_a_info = ctx.player_a_info_with_bet();
    let res = execute(
        ctx.deps.as_mut(),
        ctx.env.clone(),
        player_a_info.clone(),
        create_msg,
    )
    .unwrap();
    assert_eq!(0, res.messages.len());

    let match_id = exec::match_id(&ctx.player_a_addr, &ctx.player_b_addr, 0u64);

    let abort_msg = ExecuteMsg::AbortMatch {
        match_id: hex::encode(match_id),
    };
    let res = execute(ctx.deps.as_mut(), ctx.env, player_a_info, abort_msg).unwrap();
    assert_eq!(0, res.messages.len());

    assert_eq!(
        Ok(None),
        MATCHES.may_load(ctx.deps.as_ref().storage, match_id)
    );
    assert_eq!(
        false,
        PLAYER_MATCHES.has(ctx.deps.as_ref().storage, (&ctx.player_a_addr, match_id))
    );
    assert_eq!(
        false,
        PLAYER_MATCHES.has(ctx.deps.as_ref().storage, (&ctx.player_b_addr, match_id))
    );
    assert_eq!(
        Ok(None),
        MATCH_IDS.may_load(ctx.deps.as_ref().storage, 0u64)
    );

    let expected = Response::new()
        .add_attribute("action", "abort_match")
        .add_attribute("sender", &ctx.player_a_addr)
        .add_event(
            Event::new("match_aborted").add_attribute("match_id", hex::encode(match_id)), // Convert match_id to hexadecimal string
        );
    assert_eq!(expected, res);
}

#[test]
fn join_match_succeeds() {
    let mut ctx = TestContext::new();

    let admin_info = ctx.admin_info();
    let init_msg = InstantiateMsg {
        min_bet: Coin::new(10, NATIVE_DENOM),
    };
    let _res = instantiate(ctx.deps.as_mut(), ctx.env.clone(), admin_info, init_msg).unwrap();

    let create_msg = ExecuteMsg::CreateMatch {
        opponent: ctx.player_b_addr.clone(),
    };

    let player_a_info = ctx.player_a_info_with_bet();
    let res = execute(
        ctx.deps.as_mut(),
        ctx.env.clone(),
        player_a_info.clone(),
        create_msg,
    )
    .unwrap();
    assert_eq!(0, res.messages.len());

    let match_id = exec::match_id(&ctx.player_a_addr, &ctx.player_b_addr, 0u64);

    let abort_msg = ExecuteMsg::JoinMatch {
        match_id: hex::encode(match_id),
    };
    let player_b_info = ctx.player_b_info_with_bet();
    let res = execute(ctx.deps.as_mut(), ctx.env.clone(), player_b_info, abort_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let actual = MATCHES.load(ctx.deps.as_ref().storage, match_id).unwrap();
    let expected = Match::new_ext(
        ctx.player_a_addr.clone(),
        ctx.player_b_addr.clone(),
        MatchState::OnGoing(NextMove::Whites),
        0u64,
        0u64,
        ctx.env.block.height,
        ctx.bet.clone(),
    );
    assert_eq!(expected, actual);

    let expected = Response::new()
        .add_attribute("action", "join_match")
        .add_attribute("sender", &ctx.player_b_addr)
        .add_event(Event::new("match_started").add_attribute("match_id", hex::encode(match_id)));
    assert_eq!(expected, res);
}

#[test]
fn make_move_succeeds() {
    let mut ctx = TestContext::new();

    let admin_info = ctx.admin_info();
    let init_msg = InstantiateMsg {
        min_bet: Coin::new(10, NATIVE_DENOM),
    };
    let _res = instantiate(ctx.deps.as_mut(), ctx.env.clone(), admin_info, init_msg).unwrap();

    let create_msg = ExecuteMsg::CreateMatch {
        opponent: ctx.player_b_addr.clone(),
    };

    let player_a_info = ctx.player_a_info_with_bet();
    let res = execute(
        ctx.deps.as_mut(),
        ctx.env.clone(),
        player_a_info.clone(),
        create_msg,
    )
    .unwrap();
    assert_eq!(0, res.messages.len());

    let match_id = exec::match_id(&ctx.player_a_addr, &ctx.player_b_addr, 0u64);

    let abort_msg = ExecuteMsg::JoinMatch {
        match_id: hex::encode(match_id),
    };
    let player_b_info = ctx.player_b_info_with_bet();
    let res = execute(ctx.deps.as_mut(), ctx.env.clone(), player_b_info, abort_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let info_a_move = ctx.player_a_no_bet();
    let info_b_move = ctx.player_b_no_bet();

    let res = play_move(&mut ctx, info_a_move.clone(), match_id, "e2e4").unwrap();
    assert_eq!(0, res.messages.len());

    let actual = MATCHES.load(ctx.deps.as_ref().storage, match_id).unwrap();
    let expected = Match::new_ext(
        ctx.player_a_addr.clone(),
        ctx.player_b_addr.clone(),
        MatchState::OnGoing(NextMove::Blacks),
        0u64,
        ctx.env.block.height,
        ctx.env.block.height,
        ctx.bet.clone(),
    )
    .set_board_state("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string());
    assert_eq!(expected, actual);

    let expected = Response::new()
        .add_attribute("action", "make_move")
        .add_attribute("sender", &ctx.player_a_addr)
        .add_event(
            Event::new("move_executed")
                .add_attribute("match_id", hex::encode(match_id))
                .add_attribute("player", &ctx.player_a_addr)
                .add_attribute("move", "e2e4"),
        );
    assert_eq!(expected, res);

    assert_eq!(
        ContractError::NotYourTurn {},
        play_move(&mut ctx, info_a_move.clone(), match_id, "e7e5").unwrap_err()
    );

    assert_eq!(
        ContractError::IllegalMove {},
        play_move(&mut ctx, info_b_move.clone(), match_id, "e2e4").unwrap_err()
    );

    assert_eq!(
        ContractError::InvalidMoveEncoding {},
        play_move(&mut ctx, info_b_move.clone(), match_id, "1234").unwrap_err()
    );

    assert_eq!(
        ContractError::InvalidMoveEncoding {},
        play_move(&mut ctx, info_b_move.clone(), match_id, "e1e2e3").unwrap_err()
    );

    assert_eq!(
        ContractError::InvalidMoveEncoding {},
        play_move(&mut ctx, info_b_move.clone(), match_id, "1").unwrap_err()
    );

    // test win
    let _ = play_move(&mut ctx, info_b_move.clone(), match_id, "e7e5").unwrap();
    let _ = play_move(&mut ctx, info_a_move.clone(), match_id, "g1f3").unwrap();
    let _ = play_move(&mut ctx, info_b_move.clone(), match_id, "b8c6").unwrap();
    let _ = play_move(&mut ctx, info_a_move.clone(), match_id, "d2d4").unwrap();
    let _ = play_move(&mut ctx, info_b_move.clone(), match_id, "e5d4").unwrap();
    let _ = play_move(&mut ctx, info_a_move.clone(), match_id, "f3d4").unwrap();
    let _ = play_move(&mut ctx, info_b_move.clone(), match_id, "f8c5").unwrap();
    let _ = play_move(&mut ctx, info_a_move.clone(), match_id, "c2c3").unwrap();
    let _ = play_move(&mut ctx, info_b_move.clone(), match_id, "d8f6").unwrap();
    let _ = play_move(&mut ctx, info_a_move.clone(), match_id, "d4c6").unwrap();
    let res = play_move(&mut ctx, info_b_move.clone(), match_id, "f6f2").unwrap();

    let expected = Response::new()
        .add_attribute("action", "make_move")
        .add_attribute("sender", &ctx.player_b_addr)
        .add_event(
            Event::new("move_executed")
                .add_attribute("match_id", hex::encode(match_id))
                .add_attribute("player", &ctx.player_b_addr)
                .add_attribute("move", "f6f2"),
        )
        .add_event(
            Event::new("match_won")
                .add_attribute("match_id", hex::encode(match_id))
                .add_attribute("winner", &ctx.player_b_addr)
                .add_attribute(
                    "board",
                    "r1b1k1nr/pppp1ppp/2N5/2b5/4P3/2P5/PP3qPP/RNBQKB1R w KQkq - 0 7",
                ),
        )
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: ctx.player_b_addr.to_string(),
            amount: vec![Coin::new(ctx.bet.amount.u128() * 2, ctx.bet.denom)],
        }));
    assert_eq!(expected, res);
}

fn play_move(
    ctx: &mut TestContext,
    info: MessageInfo,
    match_id: [u8; 32],
    move_fen: &str,
) -> Result<Response, ContractError> {
    execute(
        ctx.deps.as_mut(),
        ctx.env.clone(),
        info.clone(),
        ExecuteMsg::MakeMove {
            match_id: hex::encode(match_id),
            move_fen: move_fen.to_string(),
        },
    )
}
