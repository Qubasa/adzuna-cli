use std::io::Write;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::model::{Categories, GeoData, Histogram, Job, SalaryHistory, TopCompanies, Version};

pub enum Cell {
    Text(String),
    Amount(Option<f64>),
    Count(Option<u64>),
}

impl Cell {
    fn opt(value: Option<&str>) -> Self {
        Self::Text(value.unwrap_or_default().to_owned())
    }

    fn text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Amount(n) => n.map(group_thousands).unwrap_or_default(),
            Self::Count(n) => n.map(|n| group_digits(&n.to_string())).unwrap_or_default(),
        }
    }

    fn csv(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Amount(n) => n.map(|n| n.to_string()).unwrap_or_default(),
            Self::Count(n) => n.map(|n| n.to_string()).unwrap_or_default(),
        }
    }
}

pub struct Table {
    pub headers: &'static [&'static str],
    pub rows: Vec<Vec<Cell>>,
}

/// What a command shows in `text` and `csv` formats. `json` prints the raw response instead.
pub enum View {
    Table(Table),
    Jobs {
        jobs: Vec<Job>,
        total: Option<u64>,
        currency: &'static str,
    },
}

impl View {
    pub fn write_text(self, out: &mut impl Write) -> Result<()> {
        match self {
            Self::Jobs {
                jobs,
                total,
                currency,
            } => write_jobs_text(out, &jobs, total, currency),
            Self::Table(table) => write_table_text(out, &table),
        }
    }

    pub fn write_csv(self, out: &mut impl Write) -> Result<()> {
        match self {
            Self::Jobs { jobs, currency, .. } => write_csv(out, &jobs_table(jobs, currency)),
            Self::Table(table) => write_csv(out, &table),
        }
    }
}

pub fn jobs_view(value: &Value, currency: &'static str) -> Result<View> {
    let jobs = Vec::<Job>::deserialize(&value["results"]).context("decoding search results")?;
    let total = value["count"].as_u64();
    Ok(View::Jobs {
        jobs,
        total,
        currency,
    })
}

pub fn categories_view(value: &Value) -> Result<View> {
    let categories = Categories::deserialize(value).context("decoding categories")?;
    let rows = categories
        .results
        .into_iter()
        .map(|c| {
            vec![
                Cell::Text(c.tag.unwrap_or_default()),
                Cell::Text(c.label.unwrap_or_default()),
            ]
        })
        .collect();
    Ok(View::Table(Table {
        headers: &["tag", "label"],
        rows,
    }))
}

pub fn histogram_view(value: &Value, currency: &'static str) -> Result<View> {
    let histogram = Histogram::deserialize(value).context("decoding histogram")?;
    let mut buckets: Vec<(f64, u64)> = histogram
        .histogram
        .into_iter()
        .filter_map(|(salary, ads)| Some((salary.parse().ok()?, ads)))
        .collect();
    buckets.sort_by(|a, b| a.0.total_cmp(&b.0));
    let rows = buckets
        .into_iter()
        .map(|(salary, ads)| {
            vec![
                Cell::Amount(Some(salary)),
                Cell::Count(Some(ads)),
                Cell::Text(currency.to_owned()),
            ]
        })
        .collect();
    Ok(View::Table(Table {
        headers: &["salary_from", "ads", "currency"],
        rows,
    }))
}

pub fn top_companies_view(value: &Value, currency: &'static str) -> Result<View> {
    let top = TopCompanies::deserialize(value).context("decoding top companies")?;
    let rows = top
        .leaderboard
        .into_iter()
        .map(|c| {
            let name = c.canonical_name.or(c.display_name);
            vec![
                Cell::Text(name.unwrap_or_default()),
                Cell::Count(c.count),
                Cell::Amount(c.average_salary),
                Cell::Text(currency.to_owned()),
            ]
        })
        .collect();
    Ok(View::Table(Table {
        headers: &["company", "ads", "average_salary", "currency"],
        rows,
    }))
}

pub fn geodata_view(value: &Value, currency: &'static str) -> Result<View> {
    let geo = GeoData::deserialize(value).context("decoding geodata")?;
    let rows = geo
        .locations
        .into_iter()
        .map(|l| {
            let name = l.location.and_then(|l| l.display_name);
            vec![
                Cell::Text(name.unwrap_or_default()),
                Cell::Count(l.count),
                Cell::Amount(l.average_salary),
                Cell::Text(currency.to_owned()),
            ]
        })
        .collect();
    Ok(View::Table(Table {
        headers: &["location", "ads", "average_salary", "currency"],
        rows,
    }))
}

pub fn history_view(value: &Value, currency: &'static str) -> Result<View> {
    let history = SalaryHistory::deserialize(value).context("decoding salary history")?;
    let rows = history
        .month
        .into_iter()
        .map(|(month, salary)| {
            vec![
                Cell::Text(month),
                Cell::Amount(Some(salary)),
                Cell::Text(currency.to_owned()),
            ]
        })
        .collect();
    Ok(View::Table(Table {
        headers: &["month", "average_salary", "currency"],
        rows,
    }))
}

pub fn version_view(value: &Value) -> Result<View> {
    let version = Version::deserialize(value).context("decoding version")?;
    let api = version.api_version.map(|v| v.to_string());
    let rows = vec![vec![
        Cell::opt(api.as_deref()),
        Cell::opt(version.software_version.as_deref()),
    ]];
    Ok(View::Table(Table {
        headers: &["api_version", "software_version"],
        rows,
    }))
}

fn jobs_table(jobs: Vec<Job>, currency: &'static str) -> Table {
    let mut rows = Vec::with_capacity(jobs.len());
    for job in jobs {
        let (location, area) = match job.location {
            Some(l) => (l.display_name, l.area.join(" > ")),
            None => (None, String::new()),
        };
        rows.push(vec![
            Cell::Text(job.id.unwrap_or_default()),
            Cell::Text(job.title.unwrap_or_default()),
            Cell::Text(job.company.and_then(|c| c.display_name).unwrap_or_default()),
            Cell::Text(location.unwrap_or_default()),
            Cell::Text(area),
            Cell::Text(job.category.and_then(|c| c.tag).unwrap_or_default()),
            Cell::Amount(job.salary_min),
            Cell::Amount(job.salary_max),
            Cell::Text(currency.to_owned()),
            Cell::Text(if job.salary_is_predicted { "1" } else { "0" }.to_owned()),
            Cell::Text(job.contract_type.unwrap_or_default()),
            Cell::Text(job.contract_time.unwrap_or_default()),
            Cell::Text(job.created.unwrap_or_default()),
            Cell::Amount(job.latitude),
            Cell::Amount(job.longitude),
            Cell::Text(job.redirect_url.unwrap_or_default()),
            Cell::Text(job.description.unwrap_or_default()),
        ]);
    }
    Table {
        headers: &[
            "id",
            "title",
            "company",
            "location",
            "area",
            "category",
            "salary_min",
            "salary_max",
            "currency",
            "salary_is_predicted",
            "contract_type",
            "contract_time",
            "created",
            "latitude",
            "longitude",
            "redirect_url",
            "description",
        ],
        rows,
    }
}

fn write_jobs_text(
    out: &mut impl Write,
    jobs: &[Job],
    total: Option<u64>,
    currency: &str,
) -> Result<()> {
    match total {
        Some(total) => writeln!(
            out,
            "{} of {} matching ads\n",
            jobs.len(),
            group_digits(&total.to_string())
        )?,
        None => writeln!(out, "{} ads\n", jobs.len())?,
    }
    for (index, job) in jobs.iter().enumerate() {
        writeln!(
            out,
            "{}. {}",
            index + 1,
            job.title.as_deref().unwrap_or("(untitled)")
        )?;
        let company = job.company.as_ref().and_then(|c| c.display_name.as_deref());
        let location = job
            .location
            .as_ref()
            .and_then(|l| l.display_name.as_deref());
        write_joined(out, &[company, location])?;
        let salary = salary_text(job, currency);
        let contract = [job.contract_type.as_deref(), job.contract_time.as_deref()];
        let contract = contract
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
        let posted = job.created.as_deref().and_then(|c| c.get(..10));
        write_joined(
            out,
            &[
                salary.as_deref(),
                (!contract.is_empty()).then_some(contract.as_str()),
                posted,
            ],
        )?;
        if let Some(url) = &job.redirect_url {
            writeln!(out, "   {url}")?;
        }
        writeln!(out)?;
    }
    Ok(())
}

fn write_joined(out: &mut impl Write, parts: &[Option<&str>]) -> Result<()> {
    let line = parts
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join(" | ");
    if !line.is_empty() {
        writeln!(out, "   {line}")?;
    }
    Ok(())
}

fn salary_text(job: &Job, currency: &str) -> Option<String> {
    let range = match (job.salary_min, job.salary_max) {
        (Some(min), Some(max)) if (min - max).abs() < 0.5 => group_thousands(min),
        (Some(min), Some(max)) => format!("{} - {}", group_thousands(min), group_thousands(max)),
        (Some(only), None) | (None, Some(only)) => group_thousands(only),
        (None, None) => return None,
    };
    let note = if job.salary_is_predicted {
        " (estimated)"
    } else {
        ""
    };
    Some(format!("{currency} {range}{note}"))
}

fn write_table_text(out: &mut impl Write, table: &Table) -> Result<()> {
    let rows: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| row.iter().map(Cell::text).collect())
        .collect();
    let mut widths: Vec<usize> = table.headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let headers: Vec<String> = table.headers.iter().map(|h| (*h).to_owned()).collect();
    for row in std::iter::once(&headers).chain(&rows) {
        let mut line = String::new();
        for (index, (cell, width)) in row.iter().zip(&widths).enumerate() {
            if index > 0 {
                line.push_str("  ");
            }
            line.push_str(cell);
            if index + 1 < row.len() {
                line.extend(std::iter::repeat_n(' ', width - cell.chars().count()));
            }
        }
        writeln!(out, "{}", line.trim_end())?;
    }
    Ok(())
}

fn write_csv(out: &mut impl Write, table: &Table) -> Result<()> {
    let mut writer = csv::Writer::from_writer(out);
    writer.write_record(table.headers)?;
    for row in &table.rows {
        writer.write_record(row.iter().map(Cell::csv))?;
    }
    writer.flush()?;
    Ok(())
}

/// Rounds to a whole number and inserts thousands separators: `1234567.6` -> `1,234,568`.
fn group_thousands(value: f64) -> String {
    group_digits(&format!("{value:.0}"))
}

/// Inserts thousands separators into an optionally negative integer string. Other text is kept.
fn group_digits(number: &str) -> String {
    let (sign, digits) = match number.strip_prefix('-') {
        Some(digits) => ("-", digits),
        None => ("", number),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return number.to_owned();
    }
    let mut grouped = String::with_capacity(sign.len() + digits.len() + digits.len() / 3);
    grouped.push_str(sign);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

#[expect(
    clippy::panic_in_result_fn,
    reason = "tests assert and use `?` for setup"
)]
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn text(view: View) -> Result<String> {
        let mut out = Vec::new();
        view.write_text(&mut out)?;
        Ok(String::from_utf8(out)?)
    }

    fn csv(view: View) -> Result<String> {
        let mut out = Vec::new();
        view.write_csv(&mut out)?;
        Ok(String::from_utf8(out)?)
    }

    #[test]
    fn thousands_grouping_boundaries() {
        assert_eq!(group_thousands(0.0), "0");
        assert_eq!(group_thousands(999.0), "999");
        assert_eq!(group_thousands(999.6), "1,000");
        assert_eq!(group_thousands(1_234_567.0), "1,234,567");
        assert_eq!(group_thousands(-45_000.0), "-45,000");
        assert_eq!(group_thousands(f64::NAN), "NaN");
    }

    #[test]
    fn histogram_buckets_sort_numerically_not_lexically() -> Result<()> {
        let value = json!({"histogram": {"100000": 3, "20000": 5, "9000": 1}});
        let csv = csv(histogram_view(&value, "GBP")?)?;
        assert_eq!(
            csv,
            "salary_from,ads,currency\n9000,1,GBP\n20000,5,GBP\n100000,3,GBP\n"
        );
        Ok(())
    }

    #[test]
    fn search_text_shows_salary_range_and_estimate_marker() -> Result<()> {
        let value = json!({
            "count": 1234,
            "results": [{
                "title": "Rust Engineer",
                "company": {"display_name": "Acme"},
                "location": {"display_name": "Berlin"},
                "salary_min": 70000, "salary_max": 90000, "salary_is_predicted": "1",
                "contract_type": "permanent",
                "created": "2026-11-01T10:00:00Z",
                "redirect_url": "https://example.test/ad/1"
            }]
        });
        let text = text(jobs_view(&value, "EUR")?)?;
        assert_eq!(
            text,
            "1 of 1,234 matching ads\n\n\
             1. Rust Engineer\n   Acme | Berlin\n\
             \x20  EUR 70,000 - 90,000 (estimated) | permanent | 2026-11-01\n\
             \x20  https://example.test/ad/1\n\n"
        );
        Ok(())
    }

    #[test]
    fn search_csv_quotes_fields_with_commas_and_keeps_raw_numbers() -> Result<()> {
        let value = json!({"results": [{
            "title": "Engineer, Platform",
            "salary_min": 52000.5,
            "location": {"display_name": "Soho, London", "area": ["UK", "London", "Soho"]}
        }]});
        let csv = csv(jobs_view(&value, "GBP")?)?;
        let mut reader = csv::Reader::from_reader(csv.as_bytes());
        let record = reader.records().next().context("one data row")??;
        assert_eq!(&record[1], "Engineer, Platform");
        assert_eq!(&record[3], "Soho, London");
        assert_eq!(&record[4], "UK > London > Soho");
        assert_eq!(&record[6], "52000.5");
        assert_eq!(&record[9], "0");
        Ok(())
    }

    #[test]
    fn text_table_aligns_columns_by_characters() -> Result<()> {
        let table = Table {
            headers: &["location", "ads"],
            rows: vec![
                vec![Cell::Text("Zürich".to_owned()), Cell::Count(Some(12_000))],
                vec![Cell::Text("Bern".to_owned()), Cell::Count(None)],
            ],
        };
        let text = text(View::Table(table))?;
        assert_eq!(text, "location  ads\nZürich    12,000\nBern\n");
        Ok(())
    }
}
