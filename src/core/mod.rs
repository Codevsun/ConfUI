//! Generic Value tree model and Path addressing.

#![allow(unused_imports)]

mod error;
mod path;
mod value;

pub use error::TreeError;
pub use path::{Path, PathSegment};
pub use value::Value;
