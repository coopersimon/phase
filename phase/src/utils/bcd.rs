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