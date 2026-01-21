use zerocopy::{
    FromBytes, Unaligned,
    little_endian::{I32, I64, U16, U32, U64},
};

#[repr(C)]
#[derive(FromBytes, Unaligned, Debug, Clone, Copy)]
pub struct MeteoraPoolDLMM {
    pub parameters: StaticParameters,
    pub v_parameters: VariableParameters,
    pub bump_seed: [u8; 1],
    pub bin_step_seed: [u8; 2],
    pub pair_type: u8,
    pub active_id: I32,
    pub bin_step: U16,
    pub status: u8,
    pub require_base_factor_seed: u8,
    pub base_factor_seed: [u8; 2],
    pub activation_type: u8,
    pub padding0: u8,
    pub token_x_mint: [u8; 32],
    pub token_y_mint: [u8; 32],
    pub reserve_x: [u8; 32],
    pub reserve_y: [u8; 32],
    pub protocol_fee: ProtocolFee,
    pub padding1: [u8; 256],
    pub padding2: [u8; 64],
    pub oracle: [u8; 32],
    pub bin_array_bitmap: [U64; 16],
    pub last_updated_at: I64,
    pub padding4: [u8; 96],
    pub activation_point: U64,
    pub pre_activation_duration: U64,
    pub padding5: [u8; 8],
    pub padding6: U64,
    pub padding7: [u8; 32],

    pub token_mint_x_program_flag: u8,
    pub token_mint_y_program_flag: u8,

    pub padding8: [u8; 22],
}

#[repr(C)]
#[derive(FromBytes, Unaligned, Debug, Clone, Copy)]
pub struct StaticParameters {
    pub base_factor: U16,
    pub filter_period: U16,
    pub decay_period: U16,
    pub reduction_factor: U16,
    pub variable_fee_control: U32,
    pub max_volatility_accumulator: U32,
    pub min_bin_id: I32,
    pub max_bin_id: I32,
    pub protocol_share: U16,
    pub padding: [u8; 6],
}

#[repr(C)]
#[derive(FromBytes, Unaligned, Debug, Clone, Copy)]
pub struct VariableParameters {
    pub volatility_accumulator: U32,
    pub volatility_reference: U32,
    pub index_reference: I32,
    pub padding: [u8; 4],
    pub last_update_timestamp: I64,
    pub padding1: [u8; 8],
}

#[repr(C)]
#[derive(FromBytes, Unaligned, Debug, Clone, Copy)]
pub struct ProtocolFee {
    pub amount_x: U64,
    pub amount_y: U64,
}
