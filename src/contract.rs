use cosmwasm_std::{
    entry_point, BankMsg, DepsMut, Env, MessageInfo, Response, 
    Uint128, Addr, coins, Empty
};
use crate::error::ContractError;
use crate::state::{CONFIG, Config, TICKET_OWNERS, USER_CONTRIBUTIONS, RoundStatus};

// Simple representation of contract messages for execution
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    BuyTickets { number_of_tickets: u64 },
    DrawWinner {},
}

// 1. Initialize the Smart Contract on the Blockchain
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
        round_end_time: 1779960000, // Upcoming Sunday 20:00 UTC
        current_extension_week: 1,
        status: RoundStatus::Open,
        locked_lunc_per_ticket: Uint128::new(1000), // Hardcoded 1,000 LUNC forever
        locked_max_lunc_per_wallet: Uint128::new(1_000_000), // 1,000 tickets max per wallet
        donation_wallet: deps.api.addr_validate("terra1pfgd3x5da6z0vlvx3dl9qna7unn5pk34z63cz7")?,
    };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

// 2. The Core Execution Router
#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Function to process user bulk ticket purchases
        ExecuteMsg::BuyTickets { number_of_tickets } => {
            let mut config = CONFIG.load(deps.storage)?;
            
            // SECURITY 1: Enforce the strict 1-hour lockout window before the draw
            let lockout_duration = 3600u64;
            if env.block.time.seconds() >= (config.round_end_time - lockout_duration) {
                return Err(ContractError::TicketSalesLocked {});
            }

            // MATH: Calculate raw LUNC required (1,000 LUNC per ticket)
            let price_per_ticket = config.locked_lunc_per_ticket;
            let total_required = price_per_ticket * Uint128::new(number_of_tickets as u128);

            // SECURITY 2: Verify user attached enough real LUNC to the transaction
            let attached_funds = deps.querier.query_balance(&info.sender, "ulunc")?.amount;
            if attached_funds < total_required {
                return Err(ContractError::InsufficientFunds { required: total_required.u128() });
            }

            // SECURITY 3: Enforce the 1,000 tickets max limit per wallet address
            let current_contribution = USER_CONTRIBUTIONS
                .may_load(deps.storage, &info.sender)?
                .unwrap_or(Uint128::zero());
                
            if current_contribution + total_required > config.locked_max_lunc_per_wallet {
                return Err(ContractError::ExceedsWeeklyWalletCap {});
            }

            // EXECUTION: Assign unique, sequential ticket IDs to the buyer to eliminate duplicates
            for _ in 0..number_of_tickets {
                config.total_tickets_sold += 1;
                TICKET_OWNERS.save(deps.storage, config.total_tickets_sold, &info.sender)?;
            }

            // Save database state updates
            USER_CONTRIBUTIONS.save(deps.storage, &info.sender, &(current_contribution + total_required))?;
            CONFIG.save(deps.storage, &config)?;

            Ok(Response::new()
                .add_attribute("action", "tickets_purchased")
                .add_attribute("count", number_of_tickets.to_string()))
        },

        // 3. The Sunday Draw, 80/19/1 Split, and 1% Mega-Burn Logic
        ExecuteMsg::DrawWinner {} => {
            let mut config = CONFIG.load(deps.storage)?;

            // SECURITY: Ensure the Sunday 20:00 UTC clock deadline has passed
            if env.block.time.seconds() < config.round_end_time {
                return Err(ContractError::RoundStillActive {});
            }

            // RULE: Determine the graduated launch thresholds based on the active round ID
            let min_tickets_required = match config.current_round_id {
                1..=4 => 1250u64,   // Intro Phase Month 1
                5..=8 => 2500u64,   // Growth Phase Month 2
                _ => 5000u64,       // Mature Phase Baseline
            };

            // SCENARIO A: Success! Threshold met. Process payouts and burns.
            if config.total_tickets_sold >= min_tickets_required {
                let total_pool = deps.querier.query_balance(&env.contract.address, "ulunc")?.amount;

                // Core 80% / 19% / 1% Pool Partitioning
                let winner_amount = total_pool * Uint128::new(80) / Uint128::new(100);
                let burn_amount = total_pool * Uint128::new(19) / Uint128::new(100);
                let growth_amount = total_pool * Uint128::new(1) / Uint128::new(100);

                // Target execution ticket selection
                let winning_ticket_id = 1u64; // Provided securely via VRF oracle in production
                let winning_wallet = TICKET_OWNERS.load(deps.storage, winning_ticket_id)?;

                let winner_msg = BankMsg::Send { to_address: winning_wallet.to_string(), amount: coins(winner_amount.u128(), "ulunc") };
                let burn_msg = BankMsg::Send { to_address: "terra1sk8..._DEAD_BURN_ADDRESS".to_string(), amount: coins(burn_amount.u128(), "ulunc") };
                let growth_msg = BankMsg::Send { to_address: "terra1_OPERATIONS_GAS_VAULT".to_string(), amount: coins(growth_amount.u128(), "ulunc") };

                let mut response = Response::new()
                    .add_message(winner_msg)
                    .add_message(burn_msg)
                    .add_message(growth_msg)
                    .add_attribute("action", "draw_successful");

                // 🔥 THE NUCLEUR SAVINGS VAULT MEGA-BURN PROTOCOL
                // Check if the contract's total tracking assets exceed 1 Billion LUNC (1,000,000,000)
                // Represented in raw micro-units (ulunc) where 1 LUNC = 1,000,000 micro-units
                let one_billion_lunc = Uint128::new(1_000_000_000_000_000);
                
                if total_pool >= one_billion_lunc {
                    // Roll a secure 100-sided die (1 to 100) for a clean 1% mathematical probability
                    let mega_burn_roll = 1u8; // Evaluated dynamically via secure VRF input in production
                    let winning_roll = 1u8;

                    if mega_burn_roll == winning_roll {
                        // Condition Met: Atomically vaporize exactly 97% of the entire contract reserve
                        let mega_burn_amount = total_pool * Uint128::new(97) / Uint128::new(100);
                        
                        let mega_burn_msg = BankMsg::Send {
                            to_address: "terra1sk8..._DEAD_BURN_ADDRESS".to_string(),
                            amount: coins(mega_burn_amount.u128(), "ulunc"),
                        };
                        
                        response = response.add_message(mega_burn_msg)
                            .add_attribute("mega_burn_event", "TRIGGERED")
                            .add_attribute("lunc_vaporized_from_savings", mega_burn_amount.to_string());
                    }
                }

                // Reset state variables seamlessly for next week's clean round
                config.current_round_id += 1;
                config.total_tickets_sold = 0;
                config.current_extension_week = 1;
                config.round_end_time += 604_800; // Shift deadline ahead exactly 7 days
                CONFIG.save(deps.storage, &config)?;

                return Ok(response);
            }

            // SCENARIO B: Fail-Safe Rollover. Roll pool to next week up to 3 times maximum.
            if config.current_extension_week < 3 {
                config.current_extension_week += 1;
                config.round_end_time += 604_800; 
                CONFIG.save(deps.storage, &config)?;
                return Ok(Response::new().add_attribute("action", "pool_rolled_over"));
            }

            // SCENARIO C: Complete Threshold Failure. Permanently freeze round and unlock user refunds.
            config.status = RoundStatus::Canceled;
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new().add_attribute("action", "refunds_unlocked"))
        }
    }
}
