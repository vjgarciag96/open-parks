mod client;
mod decode;
mod poller;
pub use client::{Client, FetchError};
pub use decode::{DecodeError, decode};
pub use poller::{Fetched, Latest, start};
