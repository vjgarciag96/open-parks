//! The integration tests, built as one binary so the crate links once.

mod api;
mod database;
mod last_status;
mod notification_delivery;
mod notifier;
mod open_meteo_client;
mod open_meteo_decode;
mod page;
mod park_status;
mod poller;
mod public_url;
mod schedule;
mod server;
mod sigma_client;
mod sigma_decode;
mod support;
mod telegram_bot;
mod telegram_broadcast;
mod telegram_client;
