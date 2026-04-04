use bytemuck::{Pod, Zeroable};
use orca_whirlpools_core::{AdaptiveFeeConstantsFacade, AdaptiveFeeVariablesFacade, OracleFacade};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{dex::orca::ORCA_ID, registry::ProtocolEntity};

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Oracle {
    pub whirlpool: [u8; 32],
    pub trade_enable_timestamp: u64,
    pub adaptive_fee_constants: AdaptiveFeeConstants,
    pub adaptive_fee_variables: AdaptiveFeeVariables,
    // Reserved for future use
    pub reserved: [u8; 128],
}

impl ProtocolEntity for Oracle {
    const PROGRAM_ID: Pubkey = ORCA_ID;
    const DISCRIMINATOR: &'static [u8] = &[139, 194, 131, 179, 140, 179, 229, 244];
    const DATA_SIZE: usize =
        8 + 32 + 8 + AdaptiveFeeConstants::LEN + AdaptiveFeeVariables::LEN + 128; // 254

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl Oracle {
    #[must_use]
    pub fn pubkey(&self) -> Pubkey {
        Pubkey::from(self.whirlpool)
    }
}

#[repr(C, packed)]
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Pod, Zeroable)]
pub struct AdaptiveFeeConstants {
    // Period determine high frequency trading time window
    // The unit of time is "seconds" and is applied to the chain's block time
    pub filter_period: u16,
    // Period determine when the adaptive fee start decrease
    // The unit of time is "seconds" and is applied to the chain's block time
    pub decay_period: u16,
    // Adaptive fee rate decrement rate
    pub reduction_factor: u16,
    // Used to scale the adaptive fee component
    pub adaptive_fee_control_factor: u32,
    // Maximum number of ticks crossed can be accumulated
    // Used to cap adaptive fee rate
    pub max_volatility_accumulator: u32,
    // Tick group index is defined as floor(tick_index / tick_group_size)
    pub tick_group_size: u16,
    // Major swap threshold in tick
    pub major_swap_threshold_ticks: u16,
    // Reserved for future use
    pub reserved: [u8; 16],
}

impl AdaptiveFeeConstants {
    pub const LEN: usize = 2 + 2 + 2 + 4 + 4 + 2 + 2 + 16; // 34
}

#[repr(C, packed)]
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Pod, Zeroable)]
pub struct AdaptiveFeeVariables {
    // Last timestamp (block time) when volatility_reference and tick_group_index_reference were
    // updated
    pub last_reference_update_timestamp: u64,
    // Last timestamp (block time) when major swap was executed
    pub last_major_swap_timestamp: u64,
    // Volatility reference is decayed volatility accumulator
    pub volatility_reference: u32,
    // Active tick group index of last swap
    pub tick_group_index_reference: i32,
    // Volatility accumulator measure the number of tick group crossed since reference tick group
    // index (scaled)
    pub volatility_accumulator: u32,
    // Reserved for future use
    pub reserved: [u8; 16],
}

impl AdaptiveFeeVariables {
    pub const LEN: usize = 8 + 8 + 4 + 4 + 4 + 16; // 44
}

// ---- From impls ----

impl From<&Oracle> for OracleFacade {
    fn from(o: &Oracle) -> Self {
        Self {
            trade_enable_timestamp: o.trade_enable_timestamp,
            adaptive_fee_constants: AdaptiveFeeConstantsFacade {
                filter_period: o.adaptive_fee_constants.filter_period,
                decay_period: o.adaptive_fee_constants.decay_period,
                reduction_factor: o.adaptive_fee_constants.reduction_factor,
                adaptive_fee_control_factor: o.adaptive_fee_constants.adaptive_fee_control_factor,
                max_volatility_accumulator: o.adaptive_fee_constants.max_volatility_accumulator,
                tick_group_size: o.adaptive_fee_constants.tick_group_size,
                major_swap_threshold_ticks: o.adaptive_fee_constants.major_swap_threshold_ticks,
            },
            adaptive_fee_variables: AdaptiveFeeVariablesFacade {
                last_reference_update_timestamp: o
                    .adaptive_fee_variables
                    .last_reference_update_timestamp,
                last_major_swap_timestamp: o.adaptive_fee_variables.last_major_swap_timestamp,
                volatility_reference: o.adaptive_fee_variables.volatility_reference,
                tick_group_index_reference: o.adaptive_fee_variables.tick_group_index_reference,
                volatility_accumulator: o.adaptive_fee_variables.volatility_accumulator,
            },
        }
    }
}
