use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use evaluator::{RunError, SourceProvider, Value, evaluate};

#[cfg(test)]
mod test;

#[derive(Parser, Debug)]
#[command(name = "etch", version, about = "Etch circuit language tools")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Check a program and its imports without executing them.
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
    /// Export a schematic or PCB using the shared layout for the selected format.
    Export {
        #[command(subcommand)]
        target: ExportTarget,
    },
    /// Generate an SVG schematic.
    #[command(hide = true)]
    Schematic {
        file: PathBuf,
        /// Write to this path; omit it to print SVG to stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Auto-place and autoroute a PCB, then generate an SVG preview.
    #[command(hide = true)]
    Pcb {
        file: PathBuf,
        /// Write to this path; omit it to print SVG to stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Export an editable KiCad schematic (.kicad_sch).
    #[command(hide = true)]
    KicadSchematic {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Export an editable KiCad PCB (.kicad_pcb).
    #[command(hide = true)]
    KicadPcb {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Run all registered displays and write their SVG graphs.
    Display {
        file: PathBuf,
        #[arg(short, long, default_value = "displays")]
        output_dir: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum ExportTarget {
    /// Export the logical schematic.
    Schematic {
        file: PathBuf,
        #[arg(long, value_enum)]
        format: ExportFormat,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Export the physical PCB.
    Pcb {
        file: PathBuf,
        #[arg(long, value_enum)]
        format: ExportFormat,
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ExportFormat {
    Svg,
    Kicad,
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
    Generation(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Evaluation(error) => write!(f, "{error}"),
            Self::InvalidSimulation(message) => f.write_str(message),
            Self::TestsFailed(lines) | Self::DisplaysFailed(lines) => {
                f.write_str(&lines.join("\n"))
            }
            Self::Generation(message) => f.write_str(message),
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
            evaluator::check(&sources)?;
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
        Command::Export { target } => match target {
            ExportTarget::Schematic {
                file,
                format,
                output,
            } => export_schematic(&file, format, &output),
            ExportTarget::Pcb {
                file,
                format,
                output,
            } => export_pcb(&file, format, &output),
        },
        Command::Schematic { file, output } => {
            let evaluated = evaluate_file(&file)?;
            let svg = schematic::render(&evaluated.design());
            match output {
                Some(path) => {
                    write_file(&path, &svg)?;
                    Ok(vec![format!("wrote schematic to {}", path.display())])
                }
                None => Ok(vec![svg]),
            }
        }
        Command::Pcb { file, output } => {
            let evaluated = evaluate_file(&file)?;
            let svg = pcb::render(&evaluated.design()).map_err(CliError::Generation)?;
            match output {
                Some(path) => {
                    write_file(&path, &svg)?;
                    Ok(vec![format!("wrote PCB to {}", path.display())])
                }
                None => Ok(vec![svg]),
            }
        }
        Command::KicadSchematic { file, output } => {
            export_schematic(&file, ExportFormat::Kicad, &output)
        }
        Command::KicadPcb { file, output } => export_pcb(&file, ExportFormat::Kicad, &output),
        Command::Display { file, output_dir } => {
            let evaluated = evaluate_file(&file)?;
            let displays = evaluated.run_displays();
            fs::create_dir_all(&output_dir).map_err(|error| CliError::Io {
                path: output_dir.clone(),
                error,
            })?;
            let mut lines = Vec::new();
            let mut failures = 0;
            let mut filenames = BTreeSet::new();
            for display in displays {
                match display.result {
                    Ok(graph) => {
                        let base = slug(&display.name);
                        let mut filename = format!("{base}.svg");
                        let mut suffix = 2;
                        while !filenames.insert(filename.clone()) {
                            filename = format!("{base}-{suffix}.svg");
                            suffix += 1;
                        }
                        let path = output_dir.join(filename);
                        let svg = graph_render::render(&display.name, &graph.traces);
                        write_file(&path, &svg)?;
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

fn export_schematic(
    file: &Path,
    format: ExportFormat,
    output: &Path,
) -> Result<Vec<String>, CliError> {
    match format {
        ExportFormat::Svg => validate_output_extension(output, "svg", "SVG schematic")?,
        ExportFormat::Kicad => validate_output_extension(output, "kicad_sch", "KiCad schematic")?,
    }
    let evaluated = evaluate_file(file)?;
    let contents = match format {
        ExportFormat::Svg => schematic::render(&evaluated.design()),
        ExportFormat::Kicad => {
            kicad::schematic(&evaluated.design()).map_err(CliError::Generation)?
        }
    };
    write_file(output, &contents)?;
    Ok(vec![format!("wrote schematic to {}", output.display())])
}

fn export_pcb(file: &Path, format: ExportFormat, output: &Path) -> Result<Vec<String>, CliError> {
    match format {
        ExportFormat::Svg => validate_output_extension(output, "svg", "SVG PCB")?,
        ExportFormat::Kicad => validate_output_extension(output, "kicad_pcb", "KiCad PCB")?,
    }
    let evaluated = evaluate_file(file)?;
    let contents = match format {
        ExportFormat::Svg => pcb::render(&evaluated.design()).map_err(CliError::Generation)?,
        ExportFormat::Kicad => kicad::pcb(&evaluated.design()).map_err(CliError::Generation)?,
    };
    write_file(output, &contents)?;
    Ok(vec![format!("wrote PCB to {}", output.display())])
}

fn evaluate_file(path: &Path) -> Result<evaluator::EvaluationOutput, CliError> {
    let sources = FileSources::load(path)?;
    evaluate(&sources).map_err(Into::into)
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Number(value) => value.to_string(),
        Value::Quantity(value, unit) => format!("{value} {unit}"),
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

fn validate_output_extension(
    path: &Path,
    expected_extension: &str,
    output_kind: &str,
) -> Result<(), CliError> {
    if path.extension().and_then(|extension| extension.to_str()) == Some(expected_extension) {
        return Ok(());
    }

    Err(CliError::Generation(format!(
        "{output_kind} output path must end in .{expected_extension}: {}",
        path.display()
    )))
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
    root: PathBuf,
}
impl FileSources {
    fn load(main: &Path) -> Result<Self, CliError> {
        let main = main.canonicalize().map_err(|error| CliError::Io {
            path: main.to_owned(),
            error,
        })?;
        Ok(Self {
            root: main.parent().unwrap().to_owned(),
            main: main.file_name().unwrap().to_string_lossy().into_owned(),
        })
    }
}
impl SourceProvider for FileSources {
    fn main_file(&self) -> &str {
        &self.main
    }
    fn get_file(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_file(&self, name: &str) -> Result<String, RunError> {
        let path = self.root.join(evaluator::normalize_path(name));
        fs::read_to_string(&path).map_err(|error| RunError::Io {
            file: path.display().to_string(),
            message: error.to_string(),
        })
    }
}
