#![allow(dead_code)]
#![allow(unexpected_cfgs)]

use fixed::traits::{FromFixed, ToFixed};
pub use fixed::types::U68F60 as Fraction;

#[allow(clippy::assign_op_pattern)]
#[allow(clippy::reversed_empty_ranges)]
#[allow(clippy::manual_div_ceil)]
mod uint_types {
    use uint::construct_uint;
    construct_uint! {
        pub struct U256(4);
    }
    construct_uint! {
        pub struct U128(2);
    }
}

pub trait FractionExtra {
    fn to_floor<Dst: FromFixed>(&self) -> Dst;
    fn to_ceil<Dst: FromFixed>(&self) -> Dst;
    fn to_round<Dst: FromFixed>(&self) -> Dst;
    fn from_percent<Src: ToFixed>(percent: Src) -> Self;
    fn from_bps<Src: ToFixed>(bps: Src) -> Self;
    fn to_sf(&self) -> u128;
    fn from_sf(sf: u128) -> Self;
}

impl FractionExtra for Fraction {
    #[inline]
    fn to_floor<Dst: FromFixed>(&self) -> Dst {
        self.floor().to_num()
    }

    #[inline]
    fn to_ceil<Dst: FromFixed>(&self) -> Dst {
        self.ceil().to_num()
    }

    #[inline]
    fn to_round<Dst: FromFixed>(&self) -> Dst {
        self.round().to_num()
    }

    #[inline]
    fn from_percent<Src: ToFixed>(percent: Src) -> Self {
        Self::from_num(percent) / 100
    }

    #[inline]
    fn from_bps<Src: ToFixed>(bps: Src) -> Self {
        Self::from_num(bps) / 10_000
    }

    #[inline]
    fn to_sf(&self) -> u128 {
        self.to_bits()
    }

    #[inline]
    fn from_sf(sf: u128) -> Self {
        Self::from_bits(sf)
    }
}
