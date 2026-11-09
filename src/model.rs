use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Job ad from `/jobs/{country}/search/{page}`. Every field is optional because ads omit them.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Job {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub created: Option<String>,
    pub redirect_url: Option<String>,
    pub company: Option<Company>,
    pub location: Option<Location>,
    pub category: Option<Category>,
    pub salary_min: Option<f64>,
    pub salary_max: Option<f64>,
    #[serde(deserialize_with = "flag")]
    pub salary_is_predicted: bool,
    pub contract_type: Option<String>,
    pub contract_time: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Company {
    pub display_name: Option<String>,
    pub canonical_name: Option<String>,
    pub count: Option<u64>,
    pub average_salary: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Location {
    pub display_name: Option<String>,
    pub area: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Category {
    pub tag: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Categories {
    pub results: Vec<Category>,
}

#[derive(Debug, Deserialize)]
pub struct Histogram {
    pub histogram: BTreeMap<String, u64>,
}

#[derive(Debug, Deserialize)]
pub struct TopCompanies {
    pub leaderboard: Vec<Company>,
}

#[derive(Debug, Deserialize)]
pub struct GeoData {
    pub locations: Vec<LocationJobs>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct LocationJobs {
    pub count: Option<u64>,
    pub average_salary: Option<f64>,
    pub location: Option<Location>,
}

#[derive(Debug, Deserialize)]
pub struct SalaryHistory {
    pub month: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
pub struct Version {
    pub api_version: Option<Value>,
    pub software_version: Option<String>,
}

/// Reads Adzuna's boolean flags, which the spec types as `"1"`/`"0"` but examples show as numbers.
fn flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Bool(b) => b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::String(s) => !matches_false(&s),
        Value::Null | Value::Array(_) | Value::Object(_) => false,
    })
}

fn matches_false(s: &str) -> bool {
    let s = s.trim();
    s.is_empty() || s == "0" || s.eq_ignore_ascii_case("false")
}

#[expect(
    clippy::panic_in_result_fn,
    reason = "tests assert and use `?` for setup"
)]
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn predicted(raw: Value) -> Result<bool, serde_json::Error> {
        let ad = Value::Object(serde_json::Map::from_iter([(
            "salary_is_predicted".to_owned(),
            raw,
        )]));
        Ok(Job::deserialize(&ad)?.salary_is_predicted)
    }

    #[test]
    fn salary_is_predicted_accepts_string_and_number_forms() -> Result<(), serde_json::Error> {
        assert!(predicted(json!("1"))?);
        assert!(predicted(json!(1))?);
        assert!(!predicted(json!("0"))?);
        assert!(!predicted(json!(0))?);
        assert!(!predicted(Value::Null)?);
        assert!(!Job::deserialize(&json!({}))?.salary_is_predicted);
        Ok(())
    }

    #[test]
    fn sparse_ad_still_parses() -> Result<(), serde_json::Error> {
        let job = Job::deserialize(&json!({
            "title": "Rust Engineer",
            "company": {},
            "location": {"display_name": "Berlin"},
            "salary_min": 70000,
            "__CLASS__": "Adzuna::API::Response::Job"
        }))?;
        assert_eq!(job.title.as_deref(), Some("Rust Engineer"));
        assert_eq!(job.salary_min, Some(70000.0));
        assert!(job.location.is_some_and(|l| l.area.is_empty()));
        Ok(())
    }
}
