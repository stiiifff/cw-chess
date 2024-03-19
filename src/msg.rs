use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Coin;

#[cw_serde]
pub struct InstantiateMsg {
    pub min_bet: Coin,
}

#[cw_serde]
pub enum ExecuteMsg {
    CreateMatch { opponent: String },
    AbortMatch { match_id: String },
    JoinMatch { match_id: String },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}

#[cw_serde]
pub struct MigrateMsg {}
