//! The integration tests, built as one binary so the crate links once.

mod core_database;
mod notification_delivery;
mod notifier;
mod notifier_last_status;
mod open_meteo_client;
mod open_meteo_decode;
mod open_meteo_poller;
mod park_forecast;
mod park_schedule;
mod park_status;
mod server;
mod sigma_client;
mod sigma_decode;
mod sigma_poller;
mod support;
mod telegram_bot;
mod telegram_broadcast;
mod telegram_client;
mod web_api;
mod web_page;
mod web_public_url;
