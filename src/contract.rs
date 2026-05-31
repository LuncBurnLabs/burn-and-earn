use cosmwasm_std::{
    entry_point, BankMsg, DepsMut, Env, MessageInfo, Response, 
    Uint128, coins, Empty
};
use crate::error::ContractError;
use crate::state::{CONFIG, Config, TICKET_OWNERS, USER_CONTRIBUTIONS, RoundStatus};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    BuyTickets { number_of_tickets: u64 },
    DrawWinner {},
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: Empty,
) -> Result<Response, ContractError> {
    let config = Config {
        current_round_id: 1,
        total_tickets_sold: 0,
        round_end_time: 1779960000, 
        current_extension_week: 1,
        status: RoundStatus::Open,
        locked_lunc_per_ticket: Uint128::new(1000), 
        locked_max_lunc_per_wallet: Uint128::new(1_000_000), 
        donation_wallet: deps.api.addr_validate("terra10mnynfk03p60jxmfnf0h7entqgdd5uswe5urwf")?,
    };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::BuyTickets { number_of_tickets } => {
            let mut config = CONFIG.load(deps.storage)?;
            
            let lockout_duration = 3600u64;
            if env.block.time.seconds() >= (config.round_end_time - lockout_duration) {
                return Err(ContractError::TicketSalesLocked {});
            }

            let price_per_ticket = config.locked_lunc_per_ticket;
            let total_required = price_per_ticket * Uint128::new(number_of_tickets as u128);

            let attached_funds = deps.querier.query_balance(&info.sender, "ulunc")?.amount;
            if attached_funds < total_required {
                return Err(ContractError::InsufficientFunds { required: total_required.u128() });
            }

            let current_contribution = USER_CONTRIBUTIONS
                .may_load(deps.storage, &info.sender)?
                .unwrap_or(Uint128::zero());
                
            if current_contribution + total_required > config.locked_max_lunc_per_wallet {
                return Err(ContractError::ExceedsWeeklyWalletCap {});
            }

            for _ in 0..number_of_tickets {
                config.total_tickets_sold += 1;
                TICKET_OWNERS.save(deps.storage, config.total_tickets_sold, &info.sender)?;
            }

            USER_CONTRIBUTIONS.save(deps.storage, &info.sender, &(current_contribution + total_required))?;
            CONFIG.save(deps.storage, &config)?;

            Ok(Response::new()
                .add_attribute("action", "tickets_purchased")
                .add_attribute("count", number_of_tickets.to_string()))
        },

        ExecuteMsg::DrawWinner {} => {
            let mut config = CONFIG.load(deps.storage)?;

            if env.block.time.seconds() < config.round_end_time {
                return Err(ContractError::RoundStillActive {});
            }

            let min_tickets_required = match config.current_round_id {
                1..=4 => 1250u64,   
                5..=8 => 2500u64,   
                _ => 5000u64,       
            };

            if config.total_tickets_sold >= min_tickets_required {
                let total_pool = deps.querier.query_balance(&env.contract.address, "ulunc")?.amount;

                let winner_amount = total_pool * Uint128::new(80) / Uint128::new(100);
                let burn_amount = total_pool * Uint128::new(19) / Uint128::new(100);
                let growth_amount = total_pool * Uint128::new(1) / Uint128::new(100);

                let winning_ticket_id = 1u64; 
                let winning_wallet = TICKET_OWNERS.load(deps.storage, winning_ticket_id)?;

                let winner_msg = BankMsg::Send { to_address: winning_wallet.to_string(), amount: coins(winner_amount.u128(), "ulunc") };
                let burn_msg = BankMsg::Send { to_address: "terra1sk8..._DEAD_BURN_ADDRESS".to_string(), amount: coins(burn_amount.u128(), "ulunc") };
                let growth_msg = BankMsg::Send { to_address: "terra1_OPERATIONS_GAS_VAULT".to_string(), amount: coins(growth_amount.u128(), "ulunc") };

                let mut response = Response::new()
                    .add_message(winner_msg)
                    .add_message(burn_msg)
                    .add_message(growth_msg)
                    .add_attribute("action", "draw_successful");

                let one_billion_lunc = Uint128::new(1_000_000_000_000_000);
                if total_pool >= one_billion_lunc {
                    let mega_burn_roll = ((env.block.time.nanos() % 100) + 1) as u8;
                    let winning_roll = 1u8;

                    if mega_burn_roll == winning_roll {
                        let mega_burn_amount = total_pool * Uint128::new(97) / Uint128::new(100);
                        let mega_burn_msg = BankMsg::Send {
                            to_address: "terra1sk8..._DEAD_BURN_ADDRESS".to_string(),
                            amount: coins(mega_burn_amount.u128(), "ulunc"),
                        };
                        response = response.add_message(mega_burn_msg)
                            .add_attribute("mega_burn_event", "TRIGGERED");
                    }
                }

                config.current_round_id += 1;
                config.total_tickets_sold = 0;
                config.current_extension_week = 1;
                config.round_end_time += 604_800; 
                CONFIG.save(deps.storage, &config)?;

                return Ok(response);
            }

            if config.current_extension_week < 3 {
                config.current_extension_week += 1;
                config.round_end_time += 604_800; 
                CONFIG.save(deps.storage, &config)?;
                return Ok(Response::new().add_attribute("action", "pool_rolled_over"));
            }

            config.status = RoundStatus::Canceled;
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new().add_attribute("action", "refunds_unlocked"))
        }
    }
}
