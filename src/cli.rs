use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::client::Query;
use crate::country::Country;

const LOCATION_KEYS: [&str; 8] = [
    "location0",
    "location1",
    "location2",
    "location3",
    "location4",
    "location5",
    "location6",
    "location7",
];

/// Search job ads and salary data from the Adzuna API.
///
/// Credentials come from `ADZUNA_APP_ID` / `ADZUNA_APP_KEY`, or from the config written by
/// `adzuna setup`. Each API command costs at least one request against your quota.
#[derive(Debug, Parser)]
#[command(name = "adzuna", version)]
pub struct Cli {
    /// Output format. `json` prints the API response unchanged.
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Json,
    Csv,
}

#[expect(
    clippy::large_enum_variant,
    reason = "parsed once per process, size is irrelevant"
)]
#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(flatten)]
    Api(ApiCommand),
    /// Store the app id and the command that prints the app key.
    Setup(SetupArgs),
}

/// Commands that each issue API requests.
#[expect(
    clippy::large_enum_variant,
    reason = "parsed once per process, size is irrelevant"
)]
#[derive(Debug, Subcommand)]
pub enum ApiCommand {
    /// Search job ads.
    Search(SearchArgs),
    /// List the category tags accepted by --category.
    Categories { country: Country },
    /// Salary distribution of matching ads.
    Histogram(StatsArgs),
    /// Employers with the most matching ads.
    TopCompanies(StatsArgs),
    /// Ad counts per sub-region of --location, with the `area` path to pass as --location.
    Geodata(AreaArgs),
    /// Average advertised salary per month.
    History(HistoryArgs),
    /// Show the API version (also a cheap credential check).
    Version,
}

#[derive(Debug, Args)]
pub struct AreaArgs {
    pub country: Country,

    /// Location hierarchy exactly as the `area` field of search results lists it, starting with
    /// Adzuna's name for the country: `--location UK --location London`,
    /// `--location Deutschland --location Berlin`. Up to 8 levels.
    #[arg(long = "location", value_name = "AREA")]
    pub locations: Vec<String>,

    /// Category tag from `adzuna categories`, e.g. `it-jobs`.
    #[arg(long)]
    pub category: Option<String>,
}

impl AreaArgs {
    pub fn to_query(&self) -> Result<Query> {
        if self.locations.len() > LOCATION_KEYS.len() {
            bail!(
                "at most {} --location levels are supported",
                LOCATION_KEYS.len()
            );
        }
        let mut query: Query = LOCATION_KEYS
            .into_iter()
            .zip(self.locations.iter().cloned())
            .collect();
        push(&mut query, "category", self.category.as_ref());
        Ok(query)
    }
}

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[command(flatten)]
    pub area: AreaArgs,

    /// Keywords; ads matching any of them count.
    pub keywords: Vec<String>,
}

impl StatsArgs {
    pub fn to_query(&self) -> Result<Query> {
        let mut query = self.area.to_query()?;
        push(&mut query, "what", join_words(&self.keywords).as_ref());
        Ok(query)
    }
}

#[derive(Debug, Args)]
pub struct HistoryArgs {
    #[command(flatten)]
    pub area: AreaArgs,

    /// Months of history to return.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=12))]
    pub months: Option<u8>,
}

impl HistoryArgs {
    pub fn to_query(&self) -> Result<Query> {
        let mut query = self.area.to_query()?;
        push(&mut query, "months", self.months.as_ref());
        Ok(query)
    }
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[command(flatten)]
    pub area: AreaArgs,

    /// Keywords matched in title and description; more matches rank higher.
    pub keywords: Vec<String>,

    /// Words that must all appear.
    #[arg(long, value_name = "WORDS")]
    pub all_words: Option<String>,

    /// Words of which at least one must appear.
    #[arg(long, value_name = "WORDS")]
    pub any_words: Option<String>,

    /// Exact phrase that must appear.
    #[arg(long)]
    pub phrase: Option<String>,

    /// Words that must not appear.
    #[arg(long, value_name = "WORDS")]
    pub exclude: Option<String>,

    /// Words that must appear in the title.
    #[arg(long, value_name = "WORDS")]
    pub title: Option<String>,

    /// Free-text place or postcode to search around.
    #[arg(long = "where", value_name = "PLACE")]
    pub place: Option<String>,

    /// Radius around --where in km (API default 5).
    #[arg(long, requires = "place")]
    pub distance: Option<u32>,

    /// Only ads posted within this many days.
    #[arg(long, value_name = "DAYS")]
    pub max_days_old: Option<u32>,

    /// Canonical company name, as `adzuna top-companies` reports it.
    #[arg(long)]
    pub company: Option<String>,

    /// Minimum annual salary in local currency. Excludes ads without salary unless
    /// --include-unknown-salary is set.
    #[arg(long, value_name = "AMOUNT")]
    pub salary_min: Option<u64>,

    /// Maximum annual salary in local currency.
    #[arg(long, value_name = "AMOUNT")]
    pub salary_max: Option<u64>,

    /// Keep ads without a known salary when filtering by salary.
    #[arg(long)]
    pub include_unknown_salary: bool,

    #[arg(long, value_enum)]
    pub hours: Option<Hours>,

    #[arg(long, value_enum)]
    pub contract_type: Option<ContractType>,

    #[arg(long, value_enum)]
    pub sort: Option<SortBy>,

    #[arg(long, value_enum)]
    pub sort_dir: Option<SortDir>,

    /// Number of ads to return. Each 50 ads cost one API request.
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=1000))]
    pub limit: u32,

    /// Result page to start from, counted in pages of min(--limit, 50) ads.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    pub page: u32,
}

impl SearchArgs {
    /// Query parameters except paging, which the search loop adds per request.
    pub fn to_query(&self) -> Result<Query> {
        let mut query = self.area.to_query()?;
        push(&mut query, "what", join_words(&self.keywords).as_ref());
        push(&mut query, "what_and", self.all_words.as_ref());
        push(&mut query, "what_or", self.any_words.as_ref());
        push(&mut query, "what_phrase", self.phrase.as_ref());
        push(&mut query, "what_exclude", self.exclude.as_ref());
        push(&mut query, "title_only", self.title.as_ref());
        push(&mut query, "where", self.place.as_ref());
        push(&mut query, "distance", self.distance.as_ref());
        push(&mut query, "max_days_old", self.max_days_old.as_ref());
        push(&mut query, "company", self.company.as_ref());
        push(&mut query, "salary_min", self.salary_min.as_ref());
        push(&mut query, "salary_max", self.salary_max.as_ref());
        push(&mut query, "sort_by", self.sort.map(SortBy::param).as_ref());
        push(
            &mut query,
            "sort_dir",
            self.sort_dir.map(SortDir::param).as_ref(),
        );
        if self.include_unknown_salary {
            query.push(("salary_include_unknown", "1".to_owned()));
        }
        if let Some(hours) = self.hours {
            query.push((hours.param(), "1".to_owned()));
        }
        if let Some(contract_type) = self.contract_type {
            query.push((contract_type.param(), "1".to_owned()));
        }
        Ok(query)
    }
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    /// Application ID from <https://developer.adzuna.com/admin/access_details>.
    #[arg(long)]
    pub app_id: String,

    /// Shell command that prints the application key, e.g. `rbw get adzuna-app-key`.
    #[arg(long)]
    pub app_key_command: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Hours {
    FullTime,
    PartTime,
}

impl Hours {
    fn param(self) -> &'static str {
        match self {
            Self::FullTime => "full_time",
            Self::PartTime => "part_time",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ContractType {
    Permanent,
    Contract,
}

impl ContractType {
    fn param(self) -> &'static str {
        match self {
            Self::Permanent => "permanent",
            Self::Contract => "contract",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum SortBy {
    Default,
    Hybrid,
    Date,
    Salary,
    Relevance,
}

impl SortBy {
    fn param(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Hybrid => "hybrid",
            Self::Date => "date",
            Self::Salary => "salary",
            Self::Relevance => "relevance",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum SortDir {
    Up,
    Down,
}

impl SortDir {
    fn param(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

fn push(query: &mut Query, name: &'static str, value: Option<&impl ToString>) {
    if let Some(value) = value {
        query.push((name, value.to_string()));
    }
}

fn join_words(words: &[String]) -> Option<String> {
    let joined = words.join(" ");
    let joined = joined.trim();
    (!joined.is_empty()).then(|| joined.to_owned())
}

#[expect(
    clippy::panic_in_result_fn,
    reason = "tests assert and use `?` for setup"
)]
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn parse(args: &[&str]) -> Result<Command> {
        let mut argv = vec!["adzuna"];
        argv.extend_from_slice(args);
        Ok(Cli::try_parse_from(argv)?.command)
    }

    fn query_of(args: &[&str]) -> Result<Query> {
        let Command::Api(ApiCommand::Search(search)) = parse(args)? else {
            bail!("not a search command");
        };
        search.to_query()
    }

    fn pairs(query: &Query) -> Vec<(&str, &str)> {
        query
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect()
    }

    #[test]
    fn search_filters_map_to_documented_parameter_names() -> Result<()> {
        let query = query_of(&[
            "search",
            "gb",
            "rust",
            "engineer",
            "--exclude",
            "java",
            "--where",
            "London",
            "--distance",
            "10",
            "--hours",
            "full-time",
            "--contract-type",
            "permanent",
            "--sort",
            "date",
            "--salary-min",
            "50000",
            "--include-unknown-salary",
        ])?;
        let expected = BTreeSet::from([
            ("what", "rust engineer"),
            ("what_exclude", "java"),
            ("where", "London"),
            ("distance", "10"),
            ("salary_min", "50000"),
            ("sort_by", "date"),
            ("salary_include_unknown", "1"),
            ("full_time", "1"),
            ("permanent", "1"),
        ]);
        assert_eq!(pairs(&query).into_iter().collect::<BTreeSet<_>>(), expected);
        Ok(())
    }

    #[test]
    fn locations_map_area_levels_in_order_country_first() -> Result<()> {
        let query = query_of(&[
            "search",
            "de",
            "--location",
            "Deutschland",
            "--location",
            "Berlin",
            "--location",
            "Tegel",
        ])?;
        assert_eq!(
            pairs(&query),
            [
                ("location0", "Deutschland"),
                ("location1", "Berlin"),
                ("location2", "Tegel")
            ]
        );

        let mut nine = vec!["search", "gb"];
        for _ in 0..9 {
            nine.extend(["--location", "x"]);
        }
        assert!(query_of(&nine).is_err());
        Ok(())
    }

    #[test]
    fn blank_keywords_send_no_what_parameter() -> Result<()> {
        assert_eq!(pairs(&query_of(&["search", "de", ""])?), Vec::new());
        Ok(())
    }

    #[test]
    fn rejects_unknown_country_and_out_of_range_values() {
        assert!(parse(&["search", "xx"]).is_err());
        assert!(parse(&["search", "gb", "--limit", "0"]).is_err());
        assert!(parse(&["search", "gb", "--page", "0"]).is_err());
        assert!(parse(&["history", "gb", "--months", "13"]).is_err());
        assert!(parse(&["search", "gb", "--distance", "5"]).is_err());
    }

    #[test]
    fn stats_commands_send_keywords_but_geodata_has_none() -> Result<()> {
        let Command::Api(ApiCommand::TopCompanies(args)) =
            parse(&["top-companies", "us", "data", "scientist"])?
        else {
            bail!("wrong command");
        };
        assert_eq!(pairs(&args.to_query()?), [("what", "data scientist")]);
        assert!(parse(&["geodata", "us", "data"]).is_err());
        Ok(())
    }
}
