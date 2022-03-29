#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;
#[macro_use]
extern crate prettytable;
#[macro_use]
extern crate lazy_static;

#[macro_use]
pub mod build_common;
pub mod build_ecu;
pub mod build_oas;
pub mod docker;
pub mod errors;
pub mod packages;
pub mod registry;
pub mod repl;
pub mod run;
pub mod test;
pub mod utils;



/***** CONSTANTS *****/
/// The minimum Docker version required by the Brane CLI command-line tool
pub const MIN_DOCKER_VERSION: specifications::version::Version = specifications::version::Version::new(19, 0, 0);

/// The minimum Buildx version required by the Brane CLI command-line tool
pub const MIN_BUILDX_VERSION: specifications::version::Version = specifications::version::Version::new(0, 7, 0);
