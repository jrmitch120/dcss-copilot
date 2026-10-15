//! The webtiles Game Source adapter: the only crate that knows the webtiles protocol.

mod envelope;
mod inventory;
mod json;
mod map;
mod markup;
mod messages;
mod player;
pub mod relay_protocol;
mod screens;
mod source;

pub use envelope::Envelope;
pub use source::WebtilesSource;
