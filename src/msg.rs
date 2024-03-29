use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Coin};

#[cw_serde]
pub struct InstantiateMsg {
    pub min_bet: Coin,
}

#[cw_serde]
pub enum ExecuteMsg {
    CreateMatch { opponent: Addr },
    AbortMatch { match_id: String },
    JoinMatch { match_id: String },
    MakeMove { match_id: String, move_fen: String },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}

#[cw_serde]
pub struct MigrateMsg {}
