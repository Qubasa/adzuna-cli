mod api;
mod cli;
mod client;
mod config;
mod country;
mod model;
mod output;

use std::io::{self, Write};
use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use serde_json::Value;

use crate::cli::{ApiCommand, Cli, Command, Format, SetupArgs};
use crate::client::Client;
use crate::config::{Config, EnvCredentials};
use crate::output::View;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) if is_broken_pipe(&err) => ExitCode::SUCCESS,
        Err(err) => {
            // Nothing sensible remains to do if stderr itself is gone.
            let _ = writeln!(io::stderr(), "error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let config_path = config::default_path()?;
    let command = match cli.command {
        Command::Setup(args) => return setup(&config_path, args),
        Command::Api(command) => command,
    };
    let file = config::load(&config_path)?;
    let creds = config::resolve(EnvCredentials::from_env(), file.as_ref())?;
    let client = Client::new(creds);
    let (value, view) = fetch(&client, &command)?;
    write(&value, view, cli.format)
}

fn fetch(client: &Client, command: &ApiCommand) -> Result<(Value, View)> {
    let fetched = match command {
        ApiCommand::Search(args) => {
            let value = api::search(client, args)?;
            let view = output::jobs_view(&value, args.area.country.currency())?;
            (value, view)
        }
        ApiCommand::Categories { country } => {
            let value = api::categories(client, *country)?;
            let view = output::categories_view(&value)?;
            (value, view)
        }
        ApiCommand::Histogram(args) => {
            let value = api::histogram(client, args)?;
            let view = output::histogram_view(&value, args.area.country.currency())?;
            (value, view)
        }
        ApiCommand::TopCompanies(args) => {
            let value = api::top_companies(client, args)?;
            let view = output::top_companies_view(&value)?;
            (value, view)
        }
        ApiCommand::Geodata(args) => {
            let value = api::geodata(client, args)?;
            let view = output::geodata_view(&value)?;
            (value, view)
        }
        ApiCommand::History(args) => {
            let value = api::history(client, args)?;
            let view = output::history_view(&value, args.area.country.currency())?;
            (value, view)
        }
        ApiCommand::Version => {
            let value = api::version(client)?;
            let view = output::version_view(&value)?;
            (value, view)
        }
    };
    Ok(fetched)
}

fn write(value: &Value, view: View, format: Format) -> Result<()> {
    let mut out = io::stdout().lock();
    match format {
        Format::Json => {
            serde_json::to_writer_pretty(&mut out, value)?;
            writeln!(out)?;
        }
        Format::Text => view.write_text(&mut out)?,
        Format::Csv => view.write_csv(&mut out)?,
    }
    out.flush()?;
    Ok(())
}

/// Saves the config, then proves it works with one `version` request.
fn setup(path: &std::path::Path, args: SetupArgs) -> Result<()> {
    let config = Config {
        app_id: args.app_id,
        app_key_command: args.app_key_command,
    };
    let creds = config::resolve(EnvCredentials::default(), Some(&config))?;
    let value = api::version(&Client::new(creds))?;
    config::save(path, &config)?;
    let mut out = io::stdout().lock();
    writeln!(out, "Saved {}", path.display())?;
    output::version_view(&value)?.write_text(&mut out)?;
    Ok(())
}

fn is_broken_pipe(err: &anyhow::Error) -> bool {
    err.chain()
        .filter_map(|cause| cause.downcast_ref::<io::Error>())
        .any(|io_err| io_err.kind() == io::ErrorKind::BrokenPipe)
}
