use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use reqwest::Url;
use serde_json::{Value, json};

use crate::support::fake_open_meteo;
use crate::support::fake_sigma::serving_json;
use crate::support::fixtures::{fixture, open_meteo_fixture};
use crate::support::timeouts::GENEROUS_TIMEOUT;

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_server(sigma_query_url: &Url, open_meteo_url: &Url) -> (Server, String) {
    start_server_with(sigma_query_url, open_meteo_url, &[])
}

fn start_server_with(
    sigma_query_url: &Url,
    open_meteo_url: &Url,
    envs: &[(&str, &str)],
) -> (Server, String) {
    let mut child = server_command(sigma_query_url, open_meteo_url)
        .envs(envs.iter().copied())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let server = Server(child);

    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).unwrap();
    let address = line
        .trim()
        .strip_prefix("listening on http://")
        .unwrap_or_else(|| panic!("unexpected first line: {line:?}"))
        .to_owned();
    (server, address)
}

fn server_command(sigma_query_url: &Url, open_meteo_url: &Url) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_open-parks"));
    command
        .env("OPEN_PARKS_ADDRESS", "127.0.0.1:0")
        .env("OPEN_PARKS_SIGMA_QUERY_URL", sigma_query_url.as_str())
        .env("OPEN_PARKS_OPEN_METEO_URL", open_meteo_url.as_str());
    command
}

fn open_meteo_serving_a_forecast(runtime: &tokio::runtime::Runtime) -> Url {
    runtime.block_on(fake_open_meteo::serving_json(open_meteo_fixture(
        "retiro.json",
    )))
}

fn get(address: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(address).unwrap();
    stream.set_read_timeout(Some(GENEROUS_TIMEOUT)).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn the_binary_serves_retiros_status_from_sigma_over_http() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let sigma = runtime.block_on(serving_json(fixture("code_7_closed.json")));
    let open_meteo = open_meteo_serving_a_forecast(&runtime);
    let (_server, address) = start_server(&sigma, &open_meteo);

    let response = get(&address, "/api/v1/parks/retiro/status");

    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    let mut body = serde_json::from_str::<Value>(body).unwrap();
    let fetched_at = body.as_object_mut().unwrap().remove("fetched_at");
    assert!(fetched_at.is_some_and(|value| value.is_string()), "{body}");
    let schedule = body.as_object_mut().unwrap().remove("schedule");
    assert!(
        schedule.is_some_and(|value| value["open_now"].is_boolean()),
        "{body}"
    );
    assert_eq!(
        body,
        json!({
            "code": "closed",
            "verdict": "closed",
            "dates": "13/10/2026",
            "hours": null,
            "reopening": "see_notes",
            "notes": "El parque permanecerá cerrado hasta la retirada de las ramas caídas.",
        })
    );
}

#[test]
fn the_binary_serves_retiros_status_page() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let sigma = runtime.block_on(serving_json(fixture("code_7_closed.json")));
    let open_meteo = open_meteo_serving_a_forecast(&runtime);
    let (_server, address) = start_server(&sigma, &open_meteo);

    let response = get(&address, "/");

    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    assert!(body.contains("</span> Cerrado</h1>"), "{body}");
}

#[test]
fn the_binary_serves_the_sitemap_at_its_public_url() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let sigma = runtime.block_on(serving_json(fixture("code_1_open.json")));
    let open_meteo = open_meteo_serving_a_forecast(&runtime);
    let (_server, address) = start_server_with(
        &sigma,
        &open_meteo,
        &[("OPEN_PARKS_PUBLIC_URL", "https://retiro.example")],
    );

    let response = get(&address, "/sitemap.xml");

    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    assert!(
        body.contains("<loc>https://retiro.example/</loc>"),
        "{body}"
    );
}

#[test]
fn the_binary_refuses_a_public_url_that_is_not_an_origin() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let sigma = runtime.block_on(serving_json(fixture("code_1_open.json")));
    let open_meteo = open_meteo_serving_a_forecast(&runtime);

    let output = server_command(&sigma, &open_meteo)
        .env("OPEN_PARKS_PUBLIC_URL", "https://retiro.example/es")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("OPEN_PARKS_PUBLIC_URL must be an HTTP(S) origin"),
        "{stderr}"
    );
}

#[test]
fn the_binary_polls_retiros_forecast_from_open_meteo() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let sigma = runtime.block_on(serving_json(fixture("code_1_open.json")));
    let (requested, requests) = mpsc::channel();
    let open_meteo = runtime.block_on(fake_open_meteo::serving(move || {
        let requested = requested.clone();
        async move {
            let _ = requested.send(());
            fake_open_meteo::json(open_meteo_fixture("retiro.json"))
        }
    }));

    let (_server, _) = start_server(&sigma, &open_meteo);

    requests
        .recv_timeout(GENEROUS_TIMEOUT)
        .expect("the server never asked Open-Meteo for the forecast");
}

#[test]
fn the_binary_serves_the_page_before_the_first_forecast_arrives() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let sigma = runtime.block_on(serving_json(fixture("code_1_open.json")));
    let open_meteo = runtime.block_on(fake_open_meteo::serving(std::future::pending::<()>));
    let (_server, address) = start_server(&sigma, &open_meteo);

    let response = get(&address, "/");

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
}

#[test]
fn the_binary_keeps_the_page_ok_while_the_forecast_fails() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let sigma = runtime.block_on(serving_json(fixture("code_1_open.json")));
    let open_meteo = runtime.block_on(fake_open_meteo::serving(|| async {
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    }));
    let (_server, address) = start_server(&sigma, &open_meteo);

    let response = get(&address, "/en");

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(
        response.contains("The forecast isn’t available."),
        "{response}"
    );
}

fn terminate(server: &mut Server) -> ExitStatus {
    let status = Command::new("kill")
        .args(["-TERM", &server.0.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success(), "kill: {status}");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = server.0.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "still running after SIGTERM");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_binary_exits_cleanly_on_sigterm() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let sigma = runtime.block_on(serving_json(fixture("code_1_open.json")));
    let open_meteo = open_meteo_serving_a_forecast(&runtime);
    let (mut server, address) = start_server(&sigma, &open_meteo);
    get(&address, "/api/v1/parks/retiro/status");

    assert!(terminate(&mut server).success());
}
