use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use reqwest::Url;
use serde_json::{Value, json};

mod common;
mod fake_upstream;
use common::fixture;
use fake_upstream::serving_json;

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_server(sigma_query_url: &Url) -> (Server, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_open-parks"))
        .env("OPEN_PARKS_ADDRESS", "127.0.0.1:0")
        .env("OPEN_PARKS_SIGMA_QUERY_URL", sigma_query_url.as_str())
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

fn get(address: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(address).unwrap();
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
    let (_server, address) = start_server(&sigma);

    let response = get(&address, "/api/v1/parks/retiro/status");

    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    let mut body = serde_json::from_str::<Value>(body).unwrap();
    let fetched_at = body.as_object_mut().unwrap().remove("fetched_at");
    assert!(fetched_at.is_some_and(|value| value.is_string()), "{body}");
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
    let (_server, address) = start_server(&sigma);

    let response = get(&address, "/es");

    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    assert!(body.contains("<h1>Cerrado</h1>"), "{body}");
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
    let (mut server, address) = start_server(&sigma);
    get(&address, "/api/v1/parks/retiro/status");

    assert!(terminate(&mut server).success());
}
