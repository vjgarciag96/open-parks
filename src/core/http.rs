use std::sync::Arc;

use serde::de::DeserializeOwned;

pub(crate) fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls_config())
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
}

fn tls_config() -> rustls::ClientConfig {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

/// Reads the whole body, or returns `too_large` once it's over `max_bytes`.
pub(crate) async fn read_body<E: From<reqwest::Error>>(
    response: &mut reqwest::Response,
    max_bytes: usize,
    too_large: E,
) -> Result<Vec<u8>, E> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > max_bytes {
            return Err(too_large);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub(crate) fn from_json<T: DeserializeOwned>(
    json: &str,
) -> Result<T, serde_path_to_error::Error<serde_json::Error>> {
    let mut track = serde_path_to_error::Track::new();
    let mut deserializer = serde_json::Deserializer::from_str(json);
    T::deserialize(serde_path_to_error::Deserializer::new(
        &mut deserializer,
        &mut track,
    ))
    // Reject anything after the JSON value, as `serde_json::from_str` does.
    .and_then(|value| deserializer.end().map(|()| value))
    .map_err(|error| serde_path_to_error::Error::new(track.path(), error))
}
