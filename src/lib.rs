#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub use app::{AnnatomicApp, AnnatomicArgs};
pub(crate) mod util;
