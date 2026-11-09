use clap::ValueEnum;

/// Markets served by the Adzuna API, as enumerated in its `OpenAPI` spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Country {
    At,
    Au,
    Be,
    Br,
    Ca,
    Ch,
    De,
    Es,
    Fr,
    Gb,
    In,
    It,
    Mx,
    Nl,
    Nz,
    Pl,
    Sg,
    Us,
    Za,
}

impl Country {
    pub fn code(self) -> &'static str {
        match self {
            Self::At => "at",
            Self::Au => "au",
            Self::Be => "be",
            Self::Br => "br",
            Self::Ca => "ca",
            Self::Ch => "ch",
            Self::De => "de",
            Self::Es => "es",
            Self::Fr => "fr",
            Self::Gb => "gb",
            Self::In => "in",
            Self::It => "it",
            Self::Mx => "mx",
            Self::Nl => "nl",
            Self::Nz => "nz",
            Self::Pl => "pl",
            Self::Sg => "sg",
            Self::Us => "us",
            Self::Za => "za",
        }
    }

    /// ISO 4217 code of the currency Adzuna reports salaries in for this market.
    pub fn currency(self) -> &'static str {
        match self {
            Self::At | Self::Be | Self::De | Self::Es | Self::Fr | Self::It | Self::Nl => "EUR",
            Self::Au => "AUD",
            Self::Br => "BRL",
            Self::Ca => "CAD",
            Self::Ch => "CHF",
            Self::Gb => "GBP",
            Self::In => "INR",
            Self::Mx => "MXN",
            Self::Nz => "NZD",
            Self::Pl => "PLN",
            Self::Sg => "SGD",
            Self::Us => "USD",
            Self::Za => "ZAR",
        }
    }
}
