//! Main entrypoint module for the arbitrage bot application.
extern crate arb_bot_rs as app;

use crate::entrypoint::Entrypoint;

mod entrypoint;

app::setup_app!(async { Entrypoint.run().await });
