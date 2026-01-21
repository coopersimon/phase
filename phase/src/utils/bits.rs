macro_rules! bit {
    ($bit_num:expr) => {
        1 << $bit_num
    };
}

macro_rules! bits {
    [ $($bit_num:expr),* ] => {
        $(bit!($bit_num))|*
    };
}

pub(crate) use {bit, bits};