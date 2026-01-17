macro_rules! bit {
    ($bit_num:expr) => {
        1_u32 << $bit_num
    };
}

macro_rules! bits {
    [ $($bit_num:expr),* ] => {
        $(bit!($bit_num))|*
    };
}

pub(crate) use {bit, bits};