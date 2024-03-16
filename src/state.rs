use cosmwasm_std::Addr;
use cw_storage_plus::Item;

// Contract admin address
pub const ADMIN: Item<Addr> = Item::new("contract_admin");
