use zerocopy::{FromBytes, Unaligned};

#[repr(C)]
#[derive(FromBytes, Unaligned, Debug, Clone, Copy)]
pub struct SwapMeteoraDLMM {
    // todo
}
