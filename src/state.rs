use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum RoundStatus {
    Open,
    DrawSuccessful,
    Canceled,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Config {
    pub current_round_id: u64,
    pub total_tickets_sold: u64,
    pub round_end_time: u64,
    pub current_extension_week: u8,
    pub status: RoundStatus,
    pub locked_lunc_per_ticket: Uint128,
    pub locked_max_lunc_per_wallet: Uint128,
    pub donation_wallet: Addr, // Tracks your Trust Wallet for voluntary tips
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const TICKET_OWNERS: Map<u64, Addr> = Map::new("ticket_owners");
pub const USER_CONTRIBUTIONS: Map<&Addr, Uint128> = Map::new("user_contributions");
