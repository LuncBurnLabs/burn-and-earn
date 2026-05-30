use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized: Access denied.")]
    Unauthorized {},

    #[error("Ticket sales are locked for the 1-hour countdown period before the draw.")]
    TicketSalesLocked {},

    #[error("Round is still active. Cannot draw a winner yet.")]
    RoundStillActive {},

    #[error("You have reached your 1,000 ticket maximum wallet limit for this round.")]
    ExceedsWeeklyWalletCap {},

    #[error("No ticket purchases found for this address.")]
    NoContributionFound {},

    #[error("Refunds are closed. Funds can only be reclaimed if the round fails after 3 weeks.")]
    RefundsNotAvailable {},

    #[error("Incorrect or insufficient LUNC funds attached to this transaction.")]
    InsufficientFunds { required: u128 },
}
