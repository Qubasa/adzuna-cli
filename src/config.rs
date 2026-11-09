use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const ENV_APP_ID: &str = "ADZUNA_APP_ID";
pub const ENV_APP_KEY: &str = "ADZUNA_APP_KEY";

/// On-disk settings. The key itself is never stored, only the command that prints it.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub app_id: String,
    pub app_key_command: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AppKey(String);

impl AppKey {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AppKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AppKey(<redacted>)")
    }
}

#[derive(Debug)]
pub struct Credentials {
    pub app_id: String,
    pub app_key: AppKey,
}

/// Credentials taken from the process environment, which override the config file.
#[derive(Debug, Default)]
pub struct EnvCredentials {
    pub app_id: Option<String>,
    pub app_key: Option<String>,
}

impl EnvCredentials {
    pub fn from_env() -> Self {
        let read = |name| std::env::var(name).ok().filter(|v: &String| !v.is_empty());
        Self {
            app_id: read(ENV_APP_ID),
            app_key: read(ENV_APP_KEY),
        }
    }
}

pub fn default_path() -> Result<PathBuf> {
    let Some(dir) = dirs::config_dir() else {
        bail!("cannot determine the user config directory");
    };
    Ok(dir.join("adzuna").join("config.json"))
}

pub fn load(path: &Path) -> Result<Option<Config>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
    };
    let config =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(config))
}

pub fn save(path: &Path, config: &Config) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut text = serde_json::to_string_pretty(config)?;
    text.push('\n');
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Picks the app id and key, preferring the environment over the config file.
pub fn resolve(env: EnvCredentials, config: Option<&Config>) -> Result<Credentials> {
    let app_id = match (env.app_id, config) {
        (Some(id), _) => id,
        (None, Some(config)) => config.app_id.clone(),
        (None, None) => bail!("no app id: set {ENV_APP_ID} or run `adzuna setup`"),
    };
    let app_key = match (env.app_key, config) {
        (Some(key), _) => key,
        (None, Some(config)) => run_key_command(&config.app_key_command)?,
        (None, None) => bail!("no app key: set {ENV_APP_KEY} or run `adzuna setup`"),
    };
    Ok(Credentials {
        app_id,
        app_key: AppKey(app_key),
    })
}

/// Runs `command` through `sh -c` and returns its trimmed stdout as the key.
pub fn run_key_command(command: &str) -> Result<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("running app_key_command `{command}`"))?;
    if !output.status.success() {
        bail!("app_key_command `{command}` failed with {}", output.status);
    }
    let key = String::from_utf8(output.stdout).context("app_key_command printed non-UTF-8")?;
    let key = key.trim();
    if key.is_empty() {
        bail!("app_key_command `{command}` printed nothing");
    }
    Ok(key.to_owned())
}

#[expect(
    clippy::panic_in_result_fn,
    reason = "tests assert and use `?` for setup"
)]
#[cfg(test)]
mod tests {
    use super::*;

    fn config(command: &str) -> Config {
        Config {
            app_id: "file-id".to_owned(),
            app_key_command: command.to_owned(),
        }
    }

    #[test]
    fn environment_wins_over_config_file() -> Result<()> {
        let env = EnvCredentials {
            app_id: Some("env-id".to_owned()),
            app_key: Some("env-key".to_owned()),
        };
        let creds = resolve(env, Some(&config("exit 1")))?;
        assert_eq!(creds.app_id, "env-id");
        assert_eq!(creds.app_key.expose(), "env-key");
        Ok(())
    }

    #[test]
    fn config_command_supplies_key_and_is_trimmed() -> Result<()> {
        let creds = resolve(
            EnvCredentials::default(),
            Some(&config("printf ' s3cret\\n'")),
        )?;
        assert_eq!(creds.app_id, "file-id");
        assert_eq!(creds.app_key.expose(), "s3cret");
        Ok(())
    }

    #[test]
    fn failing_or_silent_key_command_is_an_error() {
        assert!(run_key_command("exit 3").is_err());
        assert!(run_key_command("printf '\\n'").is_err());
    }

    #[test]
    fn missing_everything_points_to_setup() {
        let err = resolve(EnvCredentials::default(), None).err();
        assert!(err.is_some_and(|e| e.to_string().contains("adzuna setup")));
    }

    #[test]
    fn debug_output_never_shows_the_key() {
        let creds = Credentials {
            app_id: "id".to_owned(),
            app_key: AppKey("s3cret".to_owned()),
        };
        assert!(!format!("{creds:?}").contains("s3cret"));
    }
}
