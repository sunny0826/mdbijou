//! Shared command-line parsing for both native front ends.

use crate::config::{Config, View};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    Run(CliOptions),
    Help,
    Version,
    ListThemes,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliOptions {
    pub path: Option<PathBuf>,
    pub theme: Option<String>,
    pub edit: bool,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<CliAction, String> {
    let args: Vec<String> = args.into_iter().collect();
    let mut options = CliOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Ok(CliAction::Help),
            "-V" | "--version" => return Ok(CliAction::Version),
            "--list-themes" => return Ok(CliAction::ListThemes),
            "--edit" => options.edit = true,
            "--theme" => {
                index += 1;
                let Some(theme) = args.get(index) else {
                    return Err("missing theme after --theme".into());
                };
                options.theme = Some(theme.clone());
            }
            value if value.starts_with('-') => return Err(format!("unknown option: {value}")),
            value => options.path = Some(PathBuf::from(value)),
        }
        index += 1;
    }
    Ok(CliAction::Run(options))
}

pub fn apply_to_config(options: &CliOptions, config: &mut Config) {
    if let Some(theme) = &options.theme {
        config.theme = theme.clone();
    }
    if options.edit {
        config.default_view = View::Edit;
    }
}

pub fn validate_path(path: Option<PathBuf>) -> Result<Option<PathBuf>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    match path.try_exists() {
        Ok(true) => Ok(Some(path)),
        Ok(false) => Err(format!("file not found: {}", path.display())),
        Err(error) => Err(format!("cannot access '{}': {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_run_options() {
        assert_eq!(
            parse([
                "--edit".into(),
                "--theme".into(),
                "sepia".into(),
                "a.md".into()
            ]),
            Ok(CliAction::Run(CliOptions {
                path: Some(PathBuf::from("a.md")),
                theme: Some("sepia".into()),
                edit: true,
            }))
        );
    }

    #[test]
    fn rejects_missing_theme_value() {
        assert_eq!(
            parse(["--theme".into()]),
            Err("missing theme after --theme".into())
        );
    }
}
