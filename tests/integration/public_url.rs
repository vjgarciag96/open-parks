use open_parks::public_url::PublicUrl;
use reqwest::Url;

#[test]
fn a_public_url_is_an_http_or_https_origin() {
    for url in [
        "https://retiro.example",
        "https://retiro.example/",
        "http://localhost:3000",
    ] {
        assert!(PublicUrl::new(Url::parse(url).unwrap()).is_some(), "{url}");
    }
    for url in [
        "https://retiro.example/parks",
        "https://retiro.example/?lang=es",
        "https://retiro.example/#top",
        "https://user@retiro.example",
        "ftp://retiro.example",
        "mailto:retiro@example.org",
    ] {
        assert!(PublicUrl::new(Url::parse(url).unwrap()).is_none(), "{url}");
    }
}
