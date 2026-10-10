use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_server() -> (Server, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_open-parks"))
        .env("OPEN_PARKS_ADDRESS", "127.0.0.1:0")
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
fn the_binary_serves_retiros_status_over_http() {
    let (_server, address) = start_server();

    let response = get(&address, "/api/v1/parks/retiro/status");

    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    assert_eq!(
        serde_json::from_str::<Value>(body).unwrap(),
        json!({
            "code": "open",
            "verdict": "open",
            "dates": null,
            "hours": null,
            "reopening": null,
            "notes": null,
        })
    );
}
