use std::{env, io};

use open_parks::api;
use open_parks::park_status::{AlertCode, ParkStatus};

const DEFAULT_ADDRESS: &str = "127.0.0.1:3000";

#[tokio::main]
async fn main() -> io::Result<()> {
    let address = match env::var("OPEN_PARKS_ADDRESS") {
        Ok(address) => address,
        Err(env::VarError::NotPresent) => DEFAULT_ADDRESS.to_owned(),
        Err(error) => return Err(io::Error::other(format!("OPEN_PARKS_ADDRESS: {error}"))),
    };
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|error| io::Error::new(error.kind(), format!("binding {address}: {error}")))?;
    println!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, api::router(retiro())).await
}

// A fixed status until the live SIGMA client is wired in.
fn retiro() -> ParkStatus {
    ParkStatus {
        code: AlertCode::Open,
        dates: None,
        hours: None,
        reopening: None,
        notes: None,
    }
}
