use bytemuck::Pod;
use solana_sdk::pubkey::Pubkey;

pub trait DexEntity: Sized {
    const PROGRAM_ID: Pubkey;
    const DISCRIMINATOR: &'static [u8];
    const POOL_SIZE: usize;

    fn deserialize(data: &[u8]) -> Option<Self>;

    fn deserialize_bytemuck(data: &[u8]) -> Option<Self>
    where
        Self: Pod + Copy,
    {
        let disc_size = Self::DISCRIMINATOR.len();
        let struct_size = disc_size + size_of::<Self>();

        if data.len() < struct_size {
            return None;
        }

        if disc_size > 0 && !data.starts_with(Self::DISCRIMINATOR) {
            return None;
        }

        let payload = data.get(disc_size..)?;
        Some(bytemuck::pod_read_unaligned(payload))
    }

    fn parse_into<Out, F>(data: &[u8], wrap: F) -> Option<Out>
    where
        F: FnOnce(Box<Self>) -> Out,
    {
        Self::deserialize(data).map(|val| wrap(Box::new(val)))
    }
}
