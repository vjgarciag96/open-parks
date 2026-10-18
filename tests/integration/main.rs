//! The integration tests, built as one binary so the crate links once.

mod api;
mod database;
mod last_status;
mod notification_delivery;
mod notifier;
mod page;
mod poller;
mod schedule;
mod server;
mod sigma_client;
mod sigma_decode;
mod support;
mod telegram_bot;
mod telegram_client;
mod telegram_delivery;
mod verdict;
