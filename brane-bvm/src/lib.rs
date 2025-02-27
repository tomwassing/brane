// #[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;
#[macro_use]
extern crate num_derive;

mod builtins;
pub mod bytecode;
pub mod executor;
mod frames;
mod heap;
pub mod objects;
mod stack;
pub mod values;
pub mod vm;

pub use objects::Function;
