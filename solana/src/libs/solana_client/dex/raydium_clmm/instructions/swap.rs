use std::{collections::VecDeque, ops::Neg};

use crate::libs::solana_client::dex::raydium_clmm::{
    libraries::{U128, fixed_point_64, full_math::MulDiv, liquidity_math, swap_math, tick_math},
    *,
};

const CU_BASE: u32 = 72_250;
const CU_PER_TICK: u32 = 17_100;
const CU_SATURATION: u32 = 360_750;

// the top level state of the swap, the results of which are recorded in storage at the end
#[derive(Debug)]
pub struct SwapState {
    // the amount remaining to be swapped in/out of the input/output asset
    pub amount_specified_remaining: u64,
    // the amount already swapped out/in of the output/input asset
    pub amount_calculated: u64,
    // current sqrt(price)
    pub sqrt_price_x64: u128,
    // the tick associated with the current price
    pub tick: i32,
    // the global fee growth of the input token
    pub fee_growth_global_x64: u128,
    // the global fee of the input token
    pub fee_amount: u64,
    // amount of input token paid as protocol fee
    pub protocol_fee: u64,
    // amount of input token paid as fund fee
    pub fund_fee: u64,
    // the current liquidity in range
    pub liquidity: u128,
}

#[derive(Default)]
struct StepComputations {
    // the price at the beginning of the step
    sqrt_price_start_x64: u128,
    // the next tick to swap to from the current tick in the swap direction
    tick_next: i32,
    // whether tick_next is initialized or not
    initialized: bool,
    // sqrt(price) for the next tick (1/0)
    sqrt_price_next_x64: u128,
    // how much is being swapped in in this step
    amount_in: u64,
    // how much is being swapped out
    amount_out: u64,
    // how much fee is being paid in
    fee_amount: u64,
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::ref_option)]
pub fn swap_internal(
    amm_config: &AmmConfig,
    pool_state: &PoolState,
    tick_array_states: &mut VecDeque<&TickArrayState>,
    tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
    amount_specified: u64,
    sqrt_price_limit_x64: u128,
    zero_for_one: bool,
    is_base_input: bool,
    block_timestamp: u32,
) -> anyhow::Result<(u64, u64, u64, u32)> {
    anyhow::ensure!(amount_specified != 0, ErrorCode::ZeroAmountSpecified);
    if !pool_state.get_status_by_bit(PoolStatusBitIndex::Swap) {
        anyhow::bail!(ErrorCode::NotApproved);
    }
    if zero_for_one {
        anyhow::ensure!(
            sqrt_price_limit_x64 < pool_state.sqrt_price_x64()
                && sqrt_price_limit_x64 > tick_math::MIN_SQRT_PRICE_X64,
            ErrorCode::SqrtPriceLimitOverflow
        );
    } else {
        anyhow::ensure!(
            sqrt_price_limit_x64 > pool_state.sqrt_price_x64()
                && sqrt_price_limit_x64 < tick_math::MAX_SQRT_PRICE_X64,
            ErrorCode::SqrtPriceLimitOverflow
        );
    }

    let liquidity_start = pool_state.liquidity();

    // let updated_reward_infos = pool_state.update_reward_infos(block_timestamp as u64)?;

    let mut state = SwapState {
        amount_specified_remaining: amount_specified,
        amount_calculated: 0,
        sqrt_price_x64: pool_state.sqrt_price_x64(),
        tick: pool_state.tick_current,
        fee_growth_global_x64: if zero_for_one {
            pool_state.fee_growth_global_0_x64()
        } else {
            pool_state.fee_growth_global_1_x64()
        },
        fee_amount: 0,
        protocol_fee: 0,
        fund_fee: 0,
        liquidity: liquidity_start,
    };

    let (mut is_match_pool_current_tick_array, first_valid_tick_array_start_index) =
        pool_state.get_first_initialized_tick_array(tickarray_bitmap_extension, zero_for_one)?;
    let mut current_valid_tick_array_start_index = first_valid_tick_array_start_index;

    let mut tick_array_current = tick_array_states.pop_front().unwrap();
    // find the first active tick array account
    for _ in 0..tick_array_states.len() {
        if tick_array_current.start_tick_index == current_valid_tick_array_start_index {
            break;
        }
        tick_array_current = tick_array_states
            .pop_front()
            .ok_or(ErrorCode::NotEnoughTickArrayAccount)?;
    }
    // check first tick array account is correct
    anyhow::ensure!(
        tick_array_current.start_tick_index == current_valid_tick_array_start_index,
        "invalid first tick array account"
    );

    let mut ticks_counter: u32 = 0;

    // continue swapping as long as we haven't used the entire input/output and haven't
    // reached the price limit
    while state.amount_specified_remaining != 0 && state.sqrt_price_x64 != sqrt_price_limit_x64 {
        // Save these three pieces of information for PriceChangeEvent
        // let tick_before = state.tick;
        // let sqrt_price_x64_before = state.sqrt_price_x64;
        // let liquidity_before = state.liquidity;

        let mut step = StepComputations {
            sqrt_price_start_x64: state.sqrt_price_x64,
            ..Default::default()
        };

        let mut next_initialized_tick = if let Some(tick_state) = tick_array_current
            .next_initialized_tick(state.tick, pool_state.tick_spacing, zero_for_one)?
        {
            Box::new(*tick_state)
        } else {
            if !is_match_pool_current_tick_array {
                is_match_pool_current_tick_array = true;
                Box::new(*tick_array_current.first_initialized_tick(zero_for_one)?)
            } else {
                Box::new(TickState::default())
            }
        };
        if !next_initialized_tick.is_initialized() {
            let next_initialized_tickarray_index = pool_state
                .next_initialized_tick_array_start_index(
                    tickarray_bitmap_extension,
                    current_valid_tick_array_start_index,
                    zero_for_one,
                )?;
            if next_initialized_tickarray_index.is_none() {
                // No more initialized tick arrays available — partial swap, return
                // what has been accumulated so far instead of failing.
                break;
                // return anyhow::bail!(ErrorCode::LiquidityInsufficient);
            }

            while tick_array_current.start_tick_index != next_initialized_tickarray_index.unwrap() {
                tick_array_current = tick_array_states
                    .pop_front()
                    .ok_or(anyhow::anyhow!(ErrorCode::NotEnoughTickArrayAccount))?;
            }
            current_valid_tick_array_start_index = next_initialized_tickarray_index.unwrap();

            let first_initialized_tick = tick_array_current.first_initialized_tick(zero_for_one)?;
            *next_initialized_tick = *first_initialized_tick;
        }
        step.tick_next = next_initialized_tick.tick;
        step.initialized = next_initialized_tick.is_initialized();

        step.tick_next = step
            .tick_next
            .clamp(tick_math::MIN_TICK, tick_math::MAX_TICK);
        step.sqrt_price_next_x64 = tick_math::get_sqrt_price_at_tick(step.tick_next)?;

        let target_price = if (zero_for_one && step.sqrt_price_next_x64 < sqrt_price_limit_x64)
            || (!zero_for_one && step.sqrt_price_next_x64 > sqrt_price_limit_x64)
        {
            sqrt_price_limit_x64
        } else {
            step.sqrt_price_next_x64
        };

        if zero_for_one {
            debug_assert!(state.tick >= step.tick_next);
            debug_assert!(step.sqrt_price_start_x64 >= step.sqrt_price_next_x64);
            debug_assert!(step.sqrt_price_start_x64 >= target_price);
        } else {
            debug_assert!(step.tick_next > state.tick);
            debug_assert!(step.sqrt_price_next_x64 >= step.sqrt_price_start_x64);
            debug_assert!(target_price >= step.sqrt_price_start_x64);
        }
        let swap_step = swap_math::compute_swap_step(
            step.sqrt_price_start_x64,
            target_price,
            state.liquidity,
            state.amount_specified_remaining,
            amm_config.trade_fee_rate,
            is_base_input,
            zero_for_one,
            block_timestamp,
        )?;
        if zero_for_one {
            debug_assert!(swap_step.sqrt_price_next_x64 >= target_price);
        } else {
            debug_assert!(target_price >= swap_step.sqrt_price_next_x64);
        }
        state.sqrt_price_x64 = swap_step.sqrt_price_next_x64;
        step.amount_in = swap_step.amount_in;
        step.amount_out = swap_step.amount_out;
        step.fee_amount = swap_step.fee_amount;

        if is_base_input {
            state.amount_specified_remaining = state
                .amount_specified_remaining
                .checked_sub(step.amount_in + step.fee_amount)
                .unwrap();
            state.amount_calculated = state
                .amount_calculated
                .checked_add(step.amount_out)
                .unwrap();
        } else {
            state.amount_specified_remaining = state
                .amount_specified_remaining
                .checked_sub(step.amount_out)
                .unwrap();

            let step_amount_calculate = step
                .amount_in
                .checked_add(step.fee_amount)
                .ok_or(ErrorCode::CalculateOverflow)?;
            state.amount_calculated = state
                .amount_calculated
                .checked_add(step_amount_calculate)
                .ok_or(ErrorCode::CalculateOverflow)?;
        }

        let step_fee_amount = step.fee_amount;
        // if the protocol fee is on, calculate how much is owed, decrement fee_amount, and
        // increment protocol_fee
        if amm_config.protocol_fee_rate > 0 {
            let delta = U128::from(step_fee_amount)
                .checked_mul(amm_config.protocol_fee_rate.into())
                .unwrap()
                .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
                .unwrap()
                .as_u64();
            step.fee_amount = step.fee_amount.checked_sub(delta).unwrap();
            state.protocol_fee = state.protocol_fee.checked_add(delta).unwrap();
        }
        // if the fund fee is on, calculate how much is owed, decrement fee_amount, and increment
        // fund_fee
        if amm_config.fund_fee_rate > 0 {
            let delta = U128::from(step_fee_amount)
                .checked_mul(amm_config.fund_fee_rate.into())
                .unwrap()
                .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
                .unwrap()
                .as_u64();
            step.fee_amount = step.fee_amount.checked_sub(delta).unwrap();
            state.fund_fee = state.fund_fee.checked_add(delta).unwrap();
        }

        // update global fee tracker
        if state.liquidity > 0 {
            let fee_growth_global_x64_delta = U128::from(step.fee_amount)
                .mul_div_floor(U128::from(fixed_point_64::Q64), U128::from(state.liquidity))
                .unwrap()
                .as_u128();

            state.fee_growth_global_x64 = state
                .fee_growth_global_x64
                .checked_add(fee_growth_global_x64_delta)
                .unwrap();
            state.fee_amount = state.fee_amount.checked_add(step.fee_amount).unwrap();
        }
        // shift tick if we reached the next price
        if state.sqrt_price_x64 == step.sqrt_price_next_x64 {
            ticks_counter += 1;

            // if the tick is initialized, run the tick transition
            if step.initialized {
                // let mut liquidity_net = next_initialized_tick.cross(
                //     if zero_for_one {
                //         state.fee_growth_global_x64
                //     } else {
                //         pool_state.fee_growth_global_0_x64()
                //     },
                //     if zero_for_one {
                //         pool_state.fee_growth_global_1_x64()
                //     } else {
                //         state.fee_growth_global_x64
                //     },
                //     &updated_reward_infos,
                // );
                let mut liquidity_net = next_initialized_tick.liquidity_net();

                // update tick_state to tick_array account
                // tick_array_current.update_tick_state(
                //     next_initialized_tick.tick,
                //     pool_state.tick_spacing.into(),
                //     *next_initialized_tick,
                // )?;

                if zero_for_one {
                    liquidity_net = liquidity_net.neg();
                }
                state.liquidity = liquidity_math::add_delta(state.liquidity, liquidity_net)?;
            }

            state.tick = if zero_for_one {
                step.tick_next - 1
            } else {
                step.tick_next
            };
        } else if state.sqrt_price_x64 != step.sqrt_price_start_x64 {
            // recompute unless we're on a lower tick boundary (i.e. already transitioned ticks),
            // and haven't moved if only a small amount of quantity is traded, the input
            // may be consumed by fees, resulting in no price change. If state.sqrt_price_x64, i.e.,
            // the latest price in the pool, is used to recalculate the tick, some errors may occur.
            // for example, if zero_for_one, and the price falls exactly on an initialized tick t
            // after the first trade, then at this point, pool.sqrtPriceX64 =
            // get_sqrt_price_at_tick(t), while pool.tick = t-1. if the input quantity of the
            // second trade is very small and the pool price does not change after the transaction,
            // if the tick is recalculated, pool.tick will be equal to t, which is incorrect.
            state.tick = tick_math::get_tick_at_sqrt_price(state.sqrt_price_x64)?;
        }

        // emit!(PriceChangeEvent {
        //     pool_state: pool_state.key(),
        //     tick_before,
        //     tick_after: state.tick,
        //     sqrt_price_x64_before,
        //     sqrt_price_x64_after: state.sqrt_price_x64,
        //     liquidity_before,
        //     liquidity_after: state.liquidity,
        //     zero_for_one,
        // });
    }
    // update tick
    // if state.tick != pool_state.tick_current {
    //     // update the previous tick to the observation
    //     observation_state.update(block_timestamp, pool_state.tick_current);
    //     pool_state.tick_current = state.tick;
    // }
    // pool_state.sqrt_price_x64 = state.sqrt_price_x64;
    //
    // if liquidity_start != state.liquidity {
    //     pool_state.liquidity = state.liquidity;
    // }

    let (amount_0, amount_1) = if zero_for_one == is_base_input {
        (
            amount_specified
                .checked_sub(state.amount_specified_remaining)
                .unwrap(),
            state.amount_calculated,
        )
    } else {
        (
            state.amount_calculated,
            amount_specified
                .checked_sub(state.amount_specified_remaining)
                .unwrap(),
        )
    };

    // if zero_for_one {
    //     pool_state.fee_growth_global_0_x64 = state.fee_growth_global_x64;
    //     pool_state.total_fees_token_0 = pool_state
    //         .total_fees_token_0
    //         .checked_add(state.fee_amount)
    //         .unwrap();
    //
    //     if state.protocol_fee > 0 {
    //         pool_state.protocol_fees_token_0 = pool_state
    //             .protocol_fees_token_0
    //             .checked_add(state.protocol_fee)
    //             .unwrap();
    //     }
    //     if state.fund_fee > 0 {
    //         pool_state.fund_fees_token_0 = pool_state
    //             .fund_fees_token_0
    //             .checked_add(state.fund_fee)
    //             .unwrap();
    //     }
    //     pool_state.swap_in_amount_token_0 = pool_state
    //         .swap_in_amount_token_0
    //         .checked_add(u128::from(amount_0))
    //         .unwrap();
    //     pool_state.swap_out_amount_token_1 = pool_state
    //         .swap_out_amount_token_1
    //         .checked_add(u128::from(amount_1))
    //         .unwrap();
    // } else {
    //     pool_state.fee_growth_global_1_x64 = state.fee_growth_global_x64;
    //     pool_state.total_fees_token_1 = pool_state
    //         .total_fees_token_1
    //         .checked_add(state.fee_amount)
    //         .unwrap();
    //
    //     if state.protocol_fee > 0 {
    //         pool_state.protocol_fees_token_1 = pool_state
    //             .protocol_fees_token_1
    //             .checked_add(state.protocol_fee)
    //             .unwrap();
    //     }
    //     if state.fund_fee > 0 {
    //         pool_state.fund_fees_token_1 = pool_state
    //             .fund_fees_token_1
    //             .checked_add(state.fund_fee)
    //             .unwrap();
    //     }
    //     pool_state.swap_in_amount_token_1 = pool_state
    //         .swap_in_amount_token_1
    //         .checked_add(u128::from(amount_1))
    //         .unwrap();
    //     pool_state.swap_out_amount_token_0 = pool_state
    //         .swap_out_amount_token_0
    //         .checked_add(u128::from(amount_0))
    //         .unwrap();
    // }

    let compute_units = (CU_BASE + ticks_counter * CU_PER_TICK).min(CU_SATURATION);

    Ok((
        amount_0,
        amount_1,
        state.fee_amount + state.protocol_fee + state.fund_fee,
        compute_units,
    ))
}
