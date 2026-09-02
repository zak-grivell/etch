use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use evaluator::{RunError, SourceProvider, Value, evaluate};

#[derive(Parser, Debug)]
#[command(name = "etch", version, about = "Etch circuit language tools")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Compile and resolve a program and all of its imports.
    Check { file: PathBuf },
    /// Evaluate a program and print its resulting values.
    Run { file: PathBuf },
    /// Simulate a circuit and print every node voltage.
    Simulate {
        file: PathBuf,
        #[arg(long, default_value_t = 1)]
        steps: usize,
        #[arg(long, default_value_t = 0.001)]
        delta_time: f64,
    },
    /// Run all language tests registered by a program.
    Test { file: PathBuf },
    /// Generate an SVG schematic.
    Schematic {
        file: PathBuf,
        /// Write to this path; omit it to print SVG to stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Run all registered displays and write their SVG graphs.
    Display {
        file: PathBuf,
        #[arg(short, long, default_value = "displays")]
        output_dir: PathBuf,
    },
}

#[derive(Debug)]
pub enum CliError {
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
    Evaluation(RunError),
    InvalidSimulation(String),
    TestsFailed(Vec<String>),
    DisplaysFailed(Vec<String>),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Evaluation(error) => write!(f, "{error:?}"),
            Self::InvalidSimulation(message) => f.write_str(message),
            Self::TestsFailed(lines) | Self::DisplaysFailed(lines) => {
                f.write_str(&lines.join("\n"))
            }
        }
    }
}

impl std::error::Error for CliError {}

impl From<RunError> for CliError {
    fn from(error: RunError) -> Self {
        Self::Evaluation(error)
    }
}

pub fn execute(cli: Cli) -> Result<Vec<String>, CliError> {
    match cli.command {
        Command::Check { file } => {
            let sources = FileSources::load(&file)?;
            evaluate(&sources)?;
            Ok(vec![format!("checked {}", file.display())])
        }
        Command::Run { file } => {
            let output = evaluate_file(&file)?;
            if output.values.is_empty() {
                Ok(vec!["evaluation completed with no values".into()])
            } else {
                Ok(output
                    .values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| format!("[{index}] {}", display_value(value)))
                    .collect())
            }
        }
        Command::Simulate {
            file,
            steps,
            delta_time,
        } => {
            if steps == 0 {
                return Err(CliError::InvalidSimulation(
                    "simulation steps must be greater than zero".into(),
                ));
            }
            if !delta_time.is_finite() || delta_time <= 0.0 {
                return Err(CliError::InvalidSimulation(
                    "simulation delta time must be positive and finite".into(),
                ));
            }
            let output = evaluate_file(&file)?;
            output
                .circuit
                .simulate(steps, delta_time)
                .map_err(|error| CliError::Evaluation(RunError::Evaluation(error)))?;
            let mut lines = vec![format!(
                "simulated {steps} step(s), time={}",
                output.circuit.time()
            )];
            lines.extend(
                output
                    .circuit
                    .node_voltages()
                    .into_iter()
                    .map(|(id, voltage)| format!("node {id}: {voltage} V")),
            );
            Ok(lines)
        }
        Command::Test { file } => {
            let output = evaluate_file(&file)?;
            let results = output.run_tests();
            let mut failures = 0;
            let mut lines = Vec::new();
            for test in results {
                match test.result {
                    Ok(()) => lines.push(format!("PASS {}", test.name)),
                    Err(error) => {
                        failures += 1;
                        lines.push(format!("FAIL {}: {}", test.name, error.message));
                    }
                }
            }
            lines.push(format!(
                "{} passed; {failures} failed",
                lines.len() - failures
            ));
            if failures == 0 {
                Ok(lines)
            } else {
                Err(CliError::TestsFailed(lines))
            }
        }
        Command::Schematic { file, output } => {
            let evaluated = evaluate_file(&file)?;
            let svg = evaluated.schematic_svg();
            match output {
                Some(path) => {
                    write_file(&path, &svg)?;
                    Ok(vec![format!("wrote schematic to {}", path.display())])
                }
                None => Ok(vec![svg]),
            }
        }
        Command::Display { file, output_dir } => {
            let evaluated = evaluate_file(&file)?;
            let displays = evaluated.run_displays();
            fs::create_dir_all(&output_dir).map_err(|error| CliError::Io {
                path: output_dir.clone(),
                error,
            })?;
            let mut lines = Vec::new();
            let mut failures = 0;
            for display in displays {
                match display.result {
                    Ok(graph) => {
                        let path = output_dir.join(format!("{}.svg", slug(&display.name)));
                        write_file(&path, &graph.svg)?;
                        lines.push(format!("wrote {}", path.display()));
                    }
                    Err(error) => {
                        failures += 1;
                        lines.push(format!("FAIL {}: {}", display.name, error.message));
                    }
                }
            }
            if failures == 0 {
                Ok(lines)
            } else {
                Err(CliError::DisplaysFailed(lines))
            }
        }
    }
}

fn evaluate_file(path: &Path) -> Result<evaluator::EvaluationOutput, CliError> {
    let sources = FileSources::load(path)?;
    evaluate(&sources).map_err(Into::into)
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Number(value) => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::Boolean(value) => value.to_string(),
        Value::None => "none".into(),
        value => format!("{value:?}"),
    }
}

fn write_file(path: &Path, contents: &str) -> Result<(), CliError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| CliError::Io {
            path: parent.to_owned(),
            error,
        })?;
    }
    fs::write(path, contents).map_err(|error| CliError::Io {
        path: path.to_owned(),
        error,
    })
}

fn slug(name: &str) -> String {
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        "display".into()
    } else {
        slug
    }
}

struct FileSources {
    main: String,
    files: BTreeMap<String, String>,
}

impl FileSources {
    fn load(main: &Path) -> Result<Self, CliError> {
        let main = main.canonicalize().map_err(|error| CliError::Io {
            path: main.to_owned(),
            error,
        })?;
        let root = main.parent().unwrap_or(Path::new(".")).to_owned();
        let mut files = BTreeMap::new();
        load_directory(&root, &root, &mut files)?;
        let main = relative_name(&root, &main);
        Ok(Self { main, files })
    }
}

impl SourceProvider for FileSources {
    fn main_file(&self) -> &str {
        &self.main
    }

    fn get_file(&self, name: &str) -> Option<&str> {
        self.files.get(name).map(String::as_str)
    }
}

fn load_directory(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<(), CliError> {
    let entries = fs::read_dir(directory).map_err(|error| CliError::Io {
        path: directory.to_owned(),
        error,
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| CliError::Io {
            path: directory.to_owned(),
            error,
        })?;
        let file_type = entry.file_type().map_err(|error| CliError::Io {
            path: entry.path(),
            error,
        })?;
        if file_type.is_dir()
            && !matches!(
                entry.file_name().to_str(),
                Some("target" | ".git" | ".direnv")
            )
        {
            load_directory(root, &entry.path(), files)?;
        } else if file_type.is_file() {
            let path = entry.path();
            if matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("etch" | "txt")
            ) && let Ok(contents) = fs::read_to_string(&path)
            {
                files.insert(relative_name(root, &path), contents);
            }
        }
    }
    Ok(())
}

fn relative_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("etch-cli-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn example(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/stdlib")
            .join(name)
    }

    #[test]
    fn checks_and_runs_files_with_relative_imports() {
        let directory = TempDirectory::new();
        let main = directory.0.join("main.etch");
        fs::write(&main, "from \"module.etch\" import { answer }; answer").unwrap();
        fs::write(directory.0.join("module.etch"), "export let answer = 42").unwrap();

        let checked = execute(Cli {
            command: Command::Check { file: main.clone() },
        })
        .unwrap();
        assert!(checked[0].starts_with("checked "));
        let values = execute(Cli {
            command: Command::Run { file: main },
        })
        .unwrap();
        assert_eq!(values, ["[0] 42"]);
    }

    #[test]
    fn runs_tests_and_numeric_simulations() {
        let tests = execute(Cli {
            command: Command::Test {
                file: example("voltage_divider.etch"),
            },
        })
        .unwrap();
        assert!(tests.iter().any(|line| line.starts_with("PASS ")));

        let simulation = execute(Cli {
            command: Command::Simulate {
                file: example("voltage_divider.etch"),
                steps: 1,
                delta_time: 0.001,
            },
        })
        .unwrap();
        assert!(simulation.iter().any(|line| line.starts_with("node ")));
    }

    #[test]
    fn writes_schematic_and_display_svgs() {
        let directory = TempDirectory::new();
        let schematic = directory.0.join("schematic.svg");
        execute(Cli {
            command: Command::Schematic {
                file: example("sectioned_system.etch"),
                output: Some(schematic.clone()),
            },
        })
        .unwrap();
        assert!(fs::read_to_string(schematic).unwrap().starts_with("<svg"));

        let displays = directory.0.join("graphs");
        execute(Cli {
            command: Command::Display {
                file: example("rc_response.etch"),
                output_dir: displays.clone(),
            },
        })
        .unwrap();
        let graph = displays.join("rc-transient-response.svg");
        assert!(
            fs::read_to_string(graph)
                .unwrap()
                .contains("class=\"trace\"")
        );
    }
}
