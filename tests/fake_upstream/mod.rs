use std::net::SocketAddr;

use axum::Router;
use axum::handler::Handler;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use reqwest::Url;

pub fn url_for(address: SocketAddr) -> Url {
    Url::parse(&format!("http://{address}/layer/query")).unwrap()
}

pub async fn serving<H, T>(handler: H) -> Url
where
    H: Handler<T, ()>,
    T: 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(url_for(address).path(), get(handler));
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    url_for(address)
}

#[allow(dead_code)]
pub async fn serving_json(body: String) -> Url {
    serving(|| async move { json(body) }).await
}

pub fn json(body: String) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/json")], body)
}
