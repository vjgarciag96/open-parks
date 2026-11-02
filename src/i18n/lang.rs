#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

impl Lang {
    pub(crate) const ALL: [Lang; 2] = [Lang::Es, Lang::En];

    pub(crate) fn from_tag(tag: &str) -> Option<Lang> {
        let primary = tag.trim().split('-').next()?;
        Lang::ALL
            .into_iter()
            .find(|lang| primary.eq_ignore_ascii_case(lang.code()))
    }

    /// The ISO 639-1 code, which is also what the database stores.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    pub(crate) fn locale(self) -> &'static str {
        match self {
            Lang::Es => "es_ES",
            Lang::En => "en_GB",
        }
    }

    /// Where the page in this language is.
    pub(crate) fn path(self) -> &'static str {
        match self {
            Lang::Es => "/es",
            Lang::En => "/en",
        }
    }

    /// The language's name, in the language itself.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Lang::Es => "Español",
            Lang::En => "English",
        }
    }

    pub(crate) fn other(self) -> Lang {
        match self {
            Lang::Es => Lang::En,
            Lang::En => Lang::Es,
        }
    }
}
