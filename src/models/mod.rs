pub mod cell;
pub mod file;
pub mod game;
pub mod game_mod;
pub mod plugin;
pub mod plugin_cell;
pub mod plugin_world;
pub mod world;

pub const BATCH_SIZE: usize = 50;

use serde::Serializer;

// From: https://stackoverflow.com/a/50278316/6620612
pub fn format_radix(mut x: u64, radix: u32) -> String {
    let mut result = vec![];
    loop {
        let m = x % radix as u64;
        x /= radix as u64;

        // will panic if you use a bad radix (< 2 or > 36).
        result.push(std::char::from_digit(m as u32, radix).unwrap());
        if x == 0 {
            break;
        }
    }
    result.into_iter().rev().collect()
}

// Because JSON parsers are dumb and loose precision on i64s, serialize them to strings instead
pub fn hash_to_string<S>(hash: &i64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format_radix(*hash as u64, 36))
}
