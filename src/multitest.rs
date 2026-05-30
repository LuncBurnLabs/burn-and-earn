#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Uint128};
    use crate::contract::{instantiate, execute, ExecuteMsg};
    use crate::state::{CONFIG, RoundStatus, TICKET_OWNERS};
    use crate::error::ContractError;

    fn setup_contract(deps: cosmwasm_std::DepsMut, env: &cosmwasm_std::Env) {
        let info = mock_info("creator", &[]);
        let _res = instantiate(deps, env.clone(), info, cosmwasm_std::Empty {}).unwrap();
    }

    #[test]
    fn test_standard_purchase_and_anti_whale_cap() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        setup_contract(deps.as_mut(), &env);

        let alice_info = mock_info("alice", &coins(500_000, "ulunc"));
        deps.querier.update_balance("alice", coins(500_000, "ulunc"));
        let buy_msg = ExecuteMsg::BuyTickets { number_of_tickets: 500 };
        let res = execute(deps.as_mut(), env.clone(), alice_info, buy_msg).unwrap();
        assert!(res.attributes.iter().any(|attr| attr.key == "action" && attr.value == "tickets_purchased"));

        let alice_info_2 = mock_info("alice", &coins(600_000, "ulunc"));
        deps.querier.update_balance("alice", coins(600_000, "ulunc"));
        let buy_msg_err = ExecuteMsg::BuyTickets { number_of_tickets: 600 };
        let err = execute(deps.as_mut(), env.clone(), alice_info_2, buy_msg_err).unwrap_err();
        assert_eq!(err, ContractError::ExceedsWeeklyWalletCap {});
    }

    #[test]
    fn test_one_hour_lockout_gate() {
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        setup_contract(deps.as_mut(), &env);

        let config = CONFIG.load(&deps.storage).unwrap();
        env.block.time = env.block.time.plus_seconds(config.round_end_time - 1800);

        let bob_info = mock_info("bob", &coins(1000, "ulunc"));
        deps.querier.update_balance("bob", coins(1000, "ulunc"));
        let buy_msg = ExecuteMsg::BuyTickets { number_of_tickets: 1 };
        let err = execute(deps.as_mut(), env.clone(), bob_info, buy_msg).unwrap_err();
        assert_eq!(err, ContractError::TicketSalesLocked {});
    }

    #[test]
    fn test_low_participation_pool_rollover() {
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        setup_contract(deps.as_mut(), &env);

        let user_info = mock_info("user", &coins(10_000, "ulunc"));
        deps.querier.update_balance("user", coins(10_000, "ulunc"));
        let _res = execute(deps.as_mut(), env.clone(), user_info, ExecuteMsg::BuyTickets { number_of_tickets: 10 }).unwrap();

        let config_before = CONFIG.load(&deps.storage).unwrap();
        env.block.time = env.block.time.plus_seconds(config_before.round_end_time + 10);

        let draw_info = mock_info("automation_bot", &[]);
        let res = execute(deps.as_mut(), env.clone(), draw_info, ExecuteMsg::DrawWinner {}).unwrap();

        let has_rollover = res.attributes.iter().any(|attr| attr.key == "action" && (attr.value == "pool_rolled_over" || attr.value == "round_extended_pool_rolled_over"));
        assert!(has_rollover);
        
        let config_after = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config_after.current_extension_week, 2);
    }

    #[test]
    fn test_three_week_failure_unlocks_automated_refunds() {
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        setup_contract(deps.as_mut(), &env);

        let mut config = CONFIG.load(&deps.storage).unwrap();
        env.block.time = env.block.time.plus_seconds(config.round_end_time + 10);
        let _res = execute(deps.as_mut(), env.clone(), mock_info("bot", &[]), ExecuteMsg::DrawWinner {}).unwrap();

        config = CONFIG.load(&deps.storage).unwrap();
        env.block.time = env.block.time.plus_seconds(config.round_end_time + 10);
        let _res = execute(deps.as_mut(), env.clone(), mock_info("bot", &[]), ExecuteMsg::DrawWinner {}).unwrap();

        config = CONFIG.load(&deps.storage).unwrap();
        env.block.time = env.block.time.plus_seconds(config.round_end_time + 10);
        let res = execute(deps.as_mut(), env.clone(), mock_info("bot", &[]), ExecuteMsg::DrawWinner {}).unwrap();

        let has_refund_trigger = res.attributes.iter().any(|attr| attr.key == "action" && attr.value == "refunds_unlocked");
        assert!(has_refund_trigger);

        let config_final = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config_final.status, RoundStatus::Canceled);
    }

    #[test]
    fn test_nuclear_savings_vault_mega_burn_execution() {
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        setup_contract(deps.as_mut(), &env);

        let one_billion_lunc = Uint128::new(1_000_000_000_000_000);
        deps.querier.update_balance(env.contract.address.clone(), coins(one_billion_lunc.u128(), "ulunc"));

        let mut config = CONFIG.load(&deps.storage).unwrap();
        config.total_tickets_sold = 6000; 
        CONFIG.save(deps.as_mut().storage, &config).unwrap();
        
        TICKET_OWNERS.save(deps.as_mut().storage, 1u64, &cosmwasm_std::Addr::unchecked("lucky_winner")).unwrap();

        env.block.time = env.block.time.plus_seconds(config.round_end_time + 10);
        let res = execute(deps.as_mut(), env.clone(), mock_info("bot", &[]), ExecuteMsg::DrawWinner {}).unwrap();

        // ✨ FIXED LINE: Correctly targets index [0] of the inner BankMsg token array list
        let has_mega_burn_msg = res.messages.iter().any(|msg| {
            if let cosmwasm_std::CosmosMsg::Bank(cosmwasm_std::BankMsg::Send { to_address, amount }) = &msg.msg {
                to_address == "terra1sk8..._DEAD_BURN_ADDRESS" && amount[0].amount == Uint128::new(970_000_000_000_000)
            } else {
                false
            }
        });

        assert!(has_mega_burn_msg);
    }
}
