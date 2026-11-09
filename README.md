# adzuna-cli

`adzuna` is a command-line client for the [Adzuna API](https://developer.adzuna.com/overview):
job ads, salary distributions, top employers, regional data and salary history across 19
markets.

## Install

```bash
nix run . -- --help      # from a checkout
nix profile install .    # puts `adzuna` on PATH
```

## Credentials

Register at <https://developer.adzuna.com/signup> to get an application ID and key.
The key is never written to disk. Store a command that prints it instead:

```bash
adzuna setup --app-id 1a2b3c4d --app-key-command 'rbw get adzuna-app-key'
```

`setup` makes one `version` request to check the pair before it writes
`$XDG_CONFIG_HOME/adzuna/config.json`. `ADZUNA_APP_ID` and `ADZUNA_APP_KEY` in the environment
take precedence over the config file.

TLS certificates are checked against the operating system's trust store, so `SSL_CERT_FILE`
can add the CA of an intercepting proxy.

## Usage

Every command takes the country code first (`at au be br ca ch de es fr gb in it mx nl nz pl
sg us za`). Salaries are annual amounts in that country's currency.

```bash
adzuna search de rust engineer --where Berlin --distance 20 --hours full-time --limit 50
adzuna search gb --phrase "site reliability" --exclude java --sort date --max-days-old 7
adzuna search us python --salary-min 150000 --contract-type permanent --format csv > jobs.csv
adzuna categories gb
adzuna histogram gb devops --category it-jobs
adzuna top-companies de software engineer
adzuna geodata gb --location London
adzuna history gb --location London --category it-jobs --months 12
adzuna version
```

`--format text` (default) is for reading, `--format csv` gives one row per ad or bucket, and
`--format json` prints the API response unchanged for `jq`.

### Locations

`search --where` takes free text (a town or postcode). The statistics endpoints (`histogram`,
`top-companies`, `geodata`, `history`) only accept Adzuna's location hierarchy, outermost level
first, using the `area` values that search results report without the country:

```bash
adzuna geodata gb --location London --location "Central London"
```

`geodata` and `history` do not accept keywords; filter them with `--category`.

## Quota

Each command makes one request, except `search`, which makes one request per 50 ads
(`--limit 120` costs 3). It stops early when the results run out. Pages keep a constant size,
so `--page 3 --limit 20` returns ads 41 to 60.

## Agent skill

`skill/SKILL.md` teaches a coding agent to use the CLI. Link it into the agent's skill
directory, e.g. `ln -s "$PWD/skill" ~/.claude/skills/adzuna`.

## Development

```bash
nix develop
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo deny check
nix flake check
```
