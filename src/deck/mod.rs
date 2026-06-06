//! The `.ah-r` deck type: parsing ([`model`], [`segment`]), the interactive
//! runtime ([`app`], [`view`], [`nav`]), `<code>` highlighting ([`highlight`]),
//! and the slide [`components`] — all used only by the deck.

mod app;
mod components;
mod highlight;
mod model;
mod nav;
mod segment;
mod view;

pub(crate) use app::run;
pub(crate) use highlight::highlight_code;
pub(crate) use segment::Segment;
