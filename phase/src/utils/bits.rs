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

macro_rules! test_bit {
    ($val:expr, $bit_num:expr) => {
        ($val & bit!($bit_num)) != 0
    };
}

pub(crate) use {bit, bits, test_bit};