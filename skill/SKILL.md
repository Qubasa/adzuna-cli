---
name: adzuna
description: Search job ads and labour-market data (salary distributions, top employers, regional counts, salary history) via the Adzuna API. Use for job hunting, salary research, and employer research.
---

# Usage

The country code comes first: `at au be br ca ch de es fr gb in it mx nl nz pl sg us za`.
Salaries are annual, in that country's currency. Every command spends API quota; `search`
spends one request per 50 ads, so keep `--limit` small.

```bash
# Job ads
adzuna search de rust engineer --where Berlin --distance 20 --limit 10
adzuna search gb --phrase "platform engineer" --exclude java --sort date --max-days-old 7
adzuna search us python --salary-min 150000 --hours full-time --contract-type permanent

# Valid --category tags for a country (call before using --category)
adzuna categories gb

# Salary distribution, employers, regions, trend
adzuna histogram gb devops --category it-jobs
adzuna top-companies de software engineer
adzuna geodata gb --location UK --location London
adzuna history gb --location UK --location London --category it-jobs --months 12
```

`--format json` returns the raw API response for `jq`; `--format csv` gives one row per item.

# Gotchas

- `--where` (free text) exists only on `search`. The statistics commands take
  `--location LEVEL` repeated, one per level of an `area` path, and the first level is always
  Adzuna's country name: `--location UK --location London`,
  `--location Deutschland --location Berlin`. Without the country level the API answers
  HTTP 400. `adzuna geodata <country>` lists valid `area` paths one level deeper each time.
- `geodata` and `history` accept no keywords; narrow them with `--category`.
- `(estimated)` after a salary means Adzuna predicted it; the employer did not publish it.
- `--salary-min` drops ads without a salary unless `--include-unknown-salary` is set.

# Output Example

```
2 of 1,234 matching ads

1. Senior Rust Engineer
   Acme GmbH | Berlin
   EUR 70,000 - 90,000 (estimated) | permanent, full_time | 2026-11-01
   https://www.adzuna.de/land/ad/123
```
