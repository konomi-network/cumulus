use crate::{Config, CurrencyIdOf};
use sp_runtime::{FixedU128, FixedPointNumber};
use polkadot_parachain_primitives::{PoolId, Overflown, PriceValue};
use sp_runtime::traits::{Zero, One, CheckedMul, CheckedAdd, Saturating};
use sp_std::convert::TryInto;
use sp_std::ops::{Add, Sub, Mul};
use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::{vec::Vec};

/// The floating-rate-pool for different lending transactions.
/// Each floating-rate-pool needs to be associated with a currency id.
#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug)]
pub struct Pool<T: Config> {
    pub id: u64,
    pub name: Vec<u8>,
    pub currency_id: CurrencyIdOf<T>,

    /// indicates whether this floating-rate-pool can be used as collateral
    pub can_be_collateral: bool,
    /// This flag indicated whether this floating-rate-pool is enabled for transactions
    pub enabled: bool,

    /* --- Supply And Debt --- */
    pub supply: FixedU128,
    pub total_supply_index: FixedU128,
    pub debt: FixedU128,
    pub total_debt_index: FixedU128,
    pub interest_updated_at: T::BlockNumber,

    /* ----- Parameters ----- */
    /// Minimum amount required for each transaction and to retain the account in pool
    pub minimal_amount: FixedU128,
    /// When this is used as collateral, the value is multiplied by safe_factor
    pub safe_factor: FixedU128,
    /// Only a close_factor of the collateral can be liquidated at a time
    pub close_factor: FixedU128,
    /// The amount threshold that the arbitrageur can take all. Usually it's small.
    pub close_minimal_amount: FixedU128,
    /// The discount given to the arbitrageur
    pub discount_factor: FixedU128,
    /// The multiplier to the utilization ratio
    pub utilization_factor: FixedU128,
    /// The constant for interest rate
    pub initial_interest_rate: FixedU128,

    /* ----- Metadata Related ----- */
    /// The block number when this floating-rate-pool is last updated
    pub last_updated: T::BlockNumber,
    /// The account that lastly modified this floating-rate-pool
    pub last_updated_by: T::AccountId,
    /// The account that created this floating-rate-pool
    pub created_by: T::AccountId,
    /// The block number when this floating-rate-pool is created
    pub created_at: T::BlockNumber,
}

impl <T: Config> Pool<T> {
    /// Creates a new floating-rate-pool from the input parameters
    pub fn new(id: PoolId,
               name: Vec<u8>,
               currency_id: CurrencyIdOf<T>,
               can_be_collateral: bool,
               safe_factor: FixedU128,
               close_factor: FixedU128,
               discount_factor: FixedU128,
               utilization_factor: FixedU128,
               initial_interest_rate: FixedU128,
               minimal_amount: FixedU128,
               owner: T::AccountId,
               block_number: T::BlockNumber,
    ) -> Pool<T>{
        Pool{
            id,
            name,
            currency_id,
            can_be_collateral,
            enabled: false,
            supply: FixedU128::zero(),
            total_supply_index: FixedU128::one(),
            debt: FixedU128::zero(),
            total_debt_index: FixedU128::one(),
            interest_updated_at: T::BlockNumber::one(),
            minimal_amount,
            safe_factor,
            close_factor,
            /// 100 usd
            close_minimal_amount: FixedU128::saturating_from_integer(100),
            discount_factor,
            utilization_factor,
            initial_interest_rate,
            last_updated: block_number,
            last_updated_by: owner.clone(),
            created_by: owner,
            created_at: block_number,
        }
    }

    /// Accrue interest for the floating-rate-pool. The block_number is the block number when the floating-rate-pool is updated
    /// TODO: update and check all the overflow here
    pub fn accrue_interest(&mut self, block_number: T::BlockNumber) -> Result<(), Overflown>{
        // Not updating if the time is the same or lagging
        if self.interest_updated_at >= block_number {
            return Ok(());
        }

        // get time span
        let interval_block_number = block_number - self.interest_updated_at;
        let elapsed_time_u32 = TryInto::<u32>::try_into(interval_block_number)
            .ok()
            .expect("blockchain will not exceed 2^32 blocks; qed");

        // get rates and calculate interest
        let s_rate = self.supply_interest_rate()?;
        let d_rate = self.debt_interest_rate()?;
        let supply_multiplier = FixedU128::one() + s_rate * FixedU128::saturating_from_integer(elapsed_time_u32);
        let debt_multiplier = FixedU128::one() + d_rate * FixedU128::saturating_from_integer(elapsed_time_u32);

        self.supply = supply_multiplier * self.supply;
        self.total_supply_index = self.total_supply_index * supply_multiplier;

        self.debt = debt_multiplier * self.debt;
        self.total_debt_index = self.total_debt_index * debt_multiplier;

        self.interest_updated_at = block_number;

        Ok(())
    }

    /// Increment the supply of the pool
    pub fn increment_supply(&mut self, amount: &FixedU128) { self.supply = self.supply.add(*amount); }

    /// Decrement the supply of the pool
    /// TODO: handles if amount is more than supply
    pub fn decrement_supply(&mut self, amount: &FixedU128) { self.supply = self.supply.sub(*amount); }

    /// Increment the debt of the pool
    pub fn increment_debt(&mut self, amount: &FixedU128) { self.debt = self.debt.add(*amount); }

    /// Decrement the debt of the pool
    /// TODO: handles if amount is more than debt
    pub fn decrement_debt(&mut self, amount: &FixedU128) { self.debt = self.debt.sub(*amount); }

    /// The amount that can be close given the input
    pub fn closable_amount(&self, amount: &FixedU128, price: &PriceValue) -> FixedU128 {
        let evaluation = amount.saturating_mul(*price);
        if evaluation <= self.close_minimal_amount { return *amount; }
        self.close_factor.mul(*amount)
    }

    /// The discounted price of the pool given the current price of the currency
    pub fn discounted_price(&self, price: PriceValue) -> PriceValue { self.discount_factor * price }

    pub fn supply_interest_rate(&self) -> Result<FixedU128, Overflown> {
        if self.supply == FixedU128::zero() {
            return Ok(FixedU128::zero());
        }

        let utilization_ratio = self.debt / self.supply;
        self.debt_interest_rate()?.checked_mul(&utilization_ratio).ok_or(Overflown{})
    }

    pub fn debt_interest_rate(&self) -> Result<FixedU128, Overflown> {
        if self.supply == FixedU128::zero() {
            return Ok(self.initial_interest_rate);
        }

        let utilization_ratio = self.debt / self.supply;
        let rate = self.utilization_factor.checked_mul(&utilization_ratio).ok_or(Overflown{})?;
        self.initial_interest_rate.checked_add(&rate).ok_or(Overflown{})
    }
}