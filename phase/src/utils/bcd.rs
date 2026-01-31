/// Convert a binary number to binary coded decimal output.
/// 
/// Input must be in range 0-99 to get a valid return.
pub const fn to_bcd(binary: u8) -> Option<u8> {
    if binary > 99 {
        None
    } else {
        let tens = binary / 10;
        let units = binary % 10;
        Some((tens * 0x10) + units)
    }
}

/// Convert a number from binary coded decimal input.
/// 
/// Input must be in range 0x0-0x9, 0x10-0x19, ...
pub const fn from_bcd(bcd: u8) -> Option<u8> {
    let tens = bcd / 0x10;
    let units = bcd % 0x10;
    if tens > 0x9 || units > 0x9 {
        None
    } else {
        Some(tens * 10 + units)
    }
}