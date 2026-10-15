//! Frozen snapshot baseline from research 4d95f68; only module paths/visibility changed.
#![allow(dead_code)]
mod journal;
pub(crate) mod node;
mod packet;
mod pages;
use crate::{Result, require};
