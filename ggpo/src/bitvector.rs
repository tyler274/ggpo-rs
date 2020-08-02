pub const BITVECTOR_NIBBLE_SIZE: usize = 8;

pub fn set_bit(vector: &mut [u8], offset: &mut usize) {
    vector[((*offset) / 8)] |= 1 << ((*offset) % 8);
    *offset += 1;
}

pub fn clear_bit(vector: &mut [u8], offset: &mut usize) {
    vector[(*offset) / 8] &= !(1 << ((*offset) % 8));
    *offset += 1;
}

pub fn write_nibblet(vector: &mut [u8], nibble: usize, offset: &mut usize) {
    assert!(nibble < (1 << BITVECTOR_NIBBLE_SIZE));
    for i in 0..BITVECTOR_NIBBLE_SIZE {
        if (nibble & (1 << i)) > 0 {
            set_bit(vector, offset);
        } else {
            clear_bit(vector, offset);
        }
    }
}

pub fn read_bit(vector: &mut [u8], offset: &mut usize) -> i32 {
    let retval: i32 = !!(vector[(*offset) / 8] & (1 << ((*offset) % 8))) as i32;
    *offset += 1;
    retval
}

pub fn read_nibblet(vector: &mut [u8], offset: &mut usize) -> i32 {
    let mut nibblet: i32 = 0;
    for i in 0..BITVECTOR_NIBBLE_SIZE {
        nibblet |= read_bit(vector, offset) << i;
    }
    nibblet
}
