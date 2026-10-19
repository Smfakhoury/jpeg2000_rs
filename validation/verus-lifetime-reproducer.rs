#![cfg_attr(verus_keep_ghost, feature(proc_macro_hygiene))]

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verifier::verify)]
mod constants {
    pub const LIMIT: usize = 1;
}
