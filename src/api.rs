use anyhow::{Result, bail};
use serde_json::{Map, Value};

use crate::cli::{AreaArgs, HistoryArgs, SearchArgs, StatsArgs};
use crate::client::{Client, Query};
use crate::country::Country;

/// Largest `results_per_page` the search endpoint honours.
pub const MAX_PER_PAGE: u32 = 50;

/// Page size and request count for `limit` ads. The size stays fixed across requests
/// because the API derives each page's offset from `page * results_per_page`.
pub fn page_plan(limit: u32) -> (u32, u32) {
    let per_page = limit.min(MAX_PER_PAGE);
    (per_page, limit.div_ceil(per_page))
}

/// Fetches up to `args.limit` ads, stopping early once the API runs out of results.
/// Returns `{count, mean, results}` shaped like a single search response.
pub fn search(client: &Client, args: &SearchArgs) -> Result<Value> {
    let (per_page, pages) = page_plan(args.limit);
    let mut query = args.to_query()?;
    query.push(("results_per_page", per_page.to_string()));
    let country = args.area.country.code();

    let mut summary = Map::new();
    let mut results = Vec::new();
    for page in args.page..args.page + pages {
        let mut body = client.get(&format!("jobs/{country}/search/{page}"), &query)?;
        let Some(Value::Array(batch)) = body.get_mut("results").map(Value::take) else {
            bail!("search page {page} has no results array");
        };
        if page == args.page {
            for key in ["count", "mean"] {
                summary.insert(
                    key.to_owned(),
                    body.get_mut(key).map(Value::take).unwrap_or_default(),
                );
            }
        }
        let exhausted = batch.len() < per_page as usize;
        results.extend(batch);
        if exhausted {
            break;
        }
    }
    results.truncate(args.limit as usize);
    summary.insert("results".to_owned(), Value::Array(results));
    Ok(Value::Object(summary))
}

pub fn categories(client: &Client, country: Country) -> Result<Value> {
    client.get(
        &format!("jobs/{}/categories", country.code()),
        &Query::new(),
    )
}

pub fn histogram(client: &Client, args: &StatsArgs) -> Result<Value> {
    client.get(
        &format!("jobs/{}/histogram", args.area.country.code()),
        &args.to_query()?,
    )
}

pub fn top_companies(client: &Client, args: &StatsArgs) -> Result<Value> {
    client.get(
        &format!("jobs/{}/top_companies", args.area.country.code()),
        &args.to_query()?,
    )
}

pub fn geodata(client: &Client, args: &AreaArgs) -> Result<Value> {
    client.get(
        &format!("jobs/{}/geodata", args.country.code()),
        &args.to_query()?,
    )
}

pub fn history(client: &Client, args: &HistoryArgs) -> Result<Value> {
    client.get(
        &format!("jobs/{}/history", args.area.country.code()),
        &args.to_query()?,
    )
}

pub fn version(client: &Client) -> Result<Value> {
    client.get("version", &Query::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_plan_keeps_page_size_constant() {
        assert_eq!(page_plan(1), (1, 1));
        assert_eq!(page_plan(20), (20, 1));
        assert_eq!(page_plan(50), (50, 1));
        assert_eq!(page_plan(51), (50, 2));
        assert_eq!(page_plan(120), (50, 3));
        assert_eq!(page_plan(1000), (50, 20));
    }
}
