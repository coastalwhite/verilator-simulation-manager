#[derive(Clone, Copy)]
pub struct FuzzDimensions {
    pub bit_width: u32,
    pub num_cycles: u32,
}

impl FuzzDimensions {
    pub fn null_input(&self) -> Vec<u8> {
        let len = (self.byte_width() * self.num_cycles) as usize;
        vec![0; len]
    }

    pub fn byte_width(&self) -> u32 {
        self.bit_width.div_ceil(8)
    }
}

pub trait Mutator {
    fn num_possible_mutations(&self, dims: FuzzDimensions) -> u32;
    fn inner_apply(&mut self, dims: FuzzDimensions, idx: u32, bytes: &[u8]) -> Vec<u8>;

    #[inline]
    fn apply(&mut self, dims: FuzzDimensions, idx: u32, bytes: &[u8]) -> Vec<u8> {
        #[cfg(debug_assertions)]
        {
            assert!(bytes.len() % (dims.num_cycles as usize) == 0);
            assert!(bytes.len() % (dims.byte_width() as usize) == 0);
            assert!(idx < self.num_possible_mutations(dims));
        }

        self.inner_apply(dims, idx, bytes)
    }
}

pub struct BitFlip;
pub struct NibleFlip;
pub struct ByteFlip;

struct CloneCycle {
    max_cycles: u32,
}
struct RemoveCycle;


impl Mutator for BitFlip {
    #[inline]
    fn num_possible_mutations(&self, dims: FuzzDimensions) -> u32 {
        dims.bit_width * dims.num_cycles
    }

    fn inner_apply(&mut self, dims: FuzzDimensions, idx: u32, bytes: &[u8]) -> Vec<u8> {
        let signal = idx % dims.bit_width;
        let cycle = idx / dims.bit_width;

        let byte_offset = cycle * dims.byte_width() + signal / 8;
        let bit_offset = signal % 8;

        let mut bytes = bytes.to_vec();
        bytes[byte_offset as usize] ^= 1u8 << bit_offset;

        bytes
    }
}

impl Mutator for NibleFlip {
    #[inline]
    fn num_possible_mutations(&self, dims: FuzzDimensions) -> u32 {
        let row_nible_positions = u32::max(dims.bit_width, 3) - 3;
        row_nible_positions * dims.num_cycles
    }

    fn inner_apply(&mut self, dims: FuzzDimensions, idx: u32, bytes: &[u8]) -> Vec<u8> {
        let row_nible_positions = u32::max(dims.bit_width, 3) - 3;
        let signal = idx % row_nible_positions;
        let cycle = idx / row_nible_positions;

        let byte_width = dims.byte_width();
        let byte_offset = cycle * byte_width + signal / 8;
        let bit_offset = signal % 8;

        let mut bytes = bytes.to_vec();

        let [b0, b1] = (0b1111_0000_0000_0000u16 >> bit_offset).to_be_bytes();

        bytes[byte_offset as usize] ^= b0;
        bytes[(byte_offset + u32::from(bit_offset > 4)) as usize] ^= b1;

        bytes
    }
}

impl Mutator for ByteFlip {
    #[inline]
    fn num_possible_mutations(&self, dims: FuzzDimensions) -> u32 {
        let row_byte_positions = u32::max(dims.bit_width, 7) - 7;
        row_byte_positions * dims.num_cycles
    }

    fn inner_apply(&mut self, dims: FuzzDimensions, idx: u32, bytes: &[u8]) -> Vec<u8> {
        let row_byte_positions = u32::max(dims.bit_width, 7) - 7;
        let signal = idx % row_byte_positions;
        let cycle = idx / row_byte_positions;

        let byte_width = dims.byte_width();
        let byte_offset = cycle * byte_width + signal / 8;
        let bit_offset = signal % 8;

        let mut bytes = bytes.to_vec();

        let [b0, b1] = (0b1111_1111_0000_0000u16 >> bit_offset).to_be_bytes();

        bytes[byte_offset as usize] ^= b0;
        bytes[(byte_offset + u32::from(bit_offset > 0)) as usize] ^= b1;

        bytes
    }
}

impl Mutator for CloneCycle {
    #[inline]
    fn num_possible_mutations(&self, dims: FuzzDimensions) -> u32 {
        if dims.num_cycles >= self.max_cycles {
            return 0;
        }

        dims.num_cycles
    }

    fn inner_apply(&mut self, dims: FuzzDimensions, idx: u32, bytes: &[u8]) -> Vec<u8> {
        let byte_width = dims.byte_width();

        let mut out = Vec::with_capacity(bytes.len() + (byte_width as usize));

        let start = byte_width * (idx - 1);
        let end = start + byte_width;

        let start = start as usize;
        let end = end as usize;

        out.extend_from_slice(&bytes[..end]);
        out.extend_from_slice(&bytes[start..]);

        out
    }
}

impl Mutator for RemoveCycle {
    #[inline]
    fn num_possible_mutations(&self, dims: FuzzDimensions) -> u32 {
        u32::max(1, dims.num_cycles)
    }

    fn inner_apply(&mut self, dims: FuzzDimensions, idx: u32, bytes: &[u8]) -> Vec<u8> {
        let byte_width = dims.byte_width();

        let mut out = Vec::with_capacity(bytes.len() - (byte_width as usize));

        let start = byte_width * (idx - 1);
        let end = start + byte_width;

        let start = start as usize;
        let end = end as usize;

        out.extend_from_slice(&bytes[..start]);
        out.extend_from_slice(&bytes[end..]);

        out
    }
}
