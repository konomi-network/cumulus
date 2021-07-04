use frame_support::sp_runtime::sp_std::ops::{Div, Mul, Sub};
use frame_support::sp_runtime::traits::{CheckedMul, Zero};
use frame_support::sp_std::ops::Add;
use sp_runtime::{FixedPointNumber, FixedU128};
use sp_runtime::traits::{CheckedAdd, CheckedDiv};

use polkadot_parachain_primitives::CustomError;

/// This model is composed of two linear functions joint together.
/// The values are all represented as integers, with two decimal places. For example,
/// if the value is 80.51%, then it is represented as 8051.
pub struct TwoSegmentLinearModel {
	kink: u8,
	base: u8,
	multiplier: u8,
	jump_multiplier: u8,
}

impl Default for TwoSegmentLinearModel {
	fn default() -> Self {
		todo!()
	}
}

impl TwoSegmentLinearModel {
	pub fn debt_rate(&self, ur: &FixedU128) -> FixedU128 {
		let kink = Self::convert_num_to_fixed(&self.kink);
		let base = Self::convert_num_to_fixed(&self.base);
		let multiplier = Self::convert_num_to_fixed(&self.multiplier);
		let jump_multiplier = Self::convert_num_to_fixed(&self.jump_multiplier);

		let mut v = multiplier.mul(*ur).add(base);
		if kink.le(ur) {
			v = ur.sub(kink).mul(jump_multiplier).add(v);
		}
		v
	}

	pub fn supply_rate(&self, ur: &FixedU128) -> FixedU128 {
		self.debt_rate(ur).mul(*ur)
	}

	fn convert_num_to_fixed(num: &u8) -> FixedU128 {
		FixedU128::saturating_from_rational(num, 10000)
	}
}

/// The abstraction of different interest rate models
pub enum InterestModel {
	/// The TwoSegmentLinear model, see TwoSegmentLinearModel struct
	TwoSegmentLinear(TwoSegmentLinearModel)
}

impl InterestModel {
	pub fn debt_rate(&self, supply: &FixedU128, debt: &FixedU128) -> FixedU128 {
		let ur = self.utilization_ratio(supply, debt);
		match self {
			InterestModel::TwoSegmentLinear(model) => model.debt_rate(&ur),
		}
	}

	pub fn supply_rate(&self, supply: &FixedU128, debt: &FixedU128) -> FixedU128 {
		let ur = self.utilization_ratio(supply, debt);
		match self {
			InterestModel::TwoSegmentLinear(model) => model.supply_rate(&ur),
		}
	}

	pub fn utilization_ratio(&self, supply: &FixedU128, debt: &FixedU128) -> FixedU128 {
		if supply == FixedU128::zero() { return FixedU128::zero(); }
		debt.div(*supply)
	}
}
