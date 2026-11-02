use reqwest::Url;

/// Where the site is served, for links that leave it, such as canonical
/// links and the one in each notification.
#[derive(Debug, Clone)]
pub struct PublicUrl(Url);

impl PublicUrl {
    pub fn new(url: Url) -> Option<PublicUrl> {
        let origin = matches!(url.scheme(), "http" | "https")
            && url.has_host()
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none();
        origin.then_some(PublicUrl(url))
    }

    pub(crate) fn of(&self, path: &str) -> String {
        self.0
            .join(path)
            .expect("an HTTP(S) origin joins any absolute path")
            .into()
    }
}
