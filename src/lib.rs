//! Nimbus, a minimal weather app built with GPUI Kit.
//!
//! Read it from the data up: [`weather`] fetches and models the forecast,
//! [`system`] and [`format`] turn it into text for this user, [`theme`] and
//! [`assets`] set up the look, and [`ui`] draws the screen.

pub mod assets;
pub mod format;
pub mod system;
pub mod theme;
pub mod ui;
pub mod update;
pub mod weather;
