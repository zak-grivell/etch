use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("etch-cli-{}-{unique}", std::process::id()));
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
        .join("../examples")
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
fn writes_schematic_pcb_and_display_svgs() {
    let directory = TempDirectory::new();
    let schematic = directory.0.join("schematic.svg");
    execute(Cli {
        command: Command::Export {
            target: ExportTarget::Schematic {
                file: example("sectioned_system.etch"),
                format: ExportFormat::Svg,
                output: schematic.clone(),
            },
        },
    })
    .unwrap();
    assert!(fs::read_to_string(schematic).unwrap().starts_with("<svg"));

    let pcb = directory.0.join("pcb.svg");
    execute(Cli {
        command: Command::Export {
            target: ExportTarget::Pcb {
                file: example("pcb_voltage_divider.etch"),
                format: ExportFormat::Svg,
                output: pcb.clone(),
            },
        },
    })
    .unwrap();
    let pcb = fs::read_to_string(pcb).unwrap();
    assert!(pcb.starts_with("<svg"));
    assert!(pcb.contains("2 layer(s)"));
    assert!(pcb.contains("routed connection(s)"));

    let kicad_schematic = directory.0.join("board.kicad_sch");
    execute(Cli {
        command: Command::Export {
            target: ExportTarget::Schematic {
                file: example("pcb_voltage_divider.etch"),
                format: ExportFormat::Kicad,
                output: kicad_schematic.clone(),
            },
        },
    })
    .unwrap();
    let kicad_schematic = fs::read_to_string(kicad_schematic).unwrap();
    assert!(kicad_schematic.starts_with("(kicad_sch"));
    assert!(kicad_schematic.contains("  (label \""));
    assert!(!kicad_schematic.contains("  (wire "));

    let kicad_pcb = directory.0.join("board.kicad_pcb");
    execute(Cli {
        command: Command::Export {
            target: ExportTarget::Pcb {
                file: example("pcb_voltage_divider.etch"),
                format: ExportFormat::Kicad,
                output: kicad_pcb.clone(),
            },
        },
    })
    .unwrap();
    assert!(
        fs::read_to_string(kicad_pcb)
            .unwrap()
            .starts_with("(kicad_pcb")
    );

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

#[test]
fn rejects_incorrect_kicad_output_extensions_without_writing_files() {
    let directory = TempDirectory::new();

    let schematic = directory.0.join("board.sch");
    let error = execute(Cli {
        command: Command::KicadSchematic {
            file: example("pcb_voltage_divider.etch"),
            output: schematic.clone(),
        },
    })
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "KiCad schematic output path must end in .kicad_sch: {}",
            schematic.display()
        )
    );
    assert!(!schematic.exists());

    let pcb = directory.0.join("board.sch");
    let error = execute(Cli {
        command: Command::KicadPcb {
            file: example("pcb_voltage_divider.etch"),
            output: pcb.clone(),
        },
    })
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "KiCad PCB output path must end in .kicad_pcb: {}",
            pcb.display()
        )
    );
    assert!(!pcb.exists());
}

#[test]
fn preserves_displays_with_colliding_filenames() {
    let directory = TempDirectory::new();
    let main = directory.0.join("main.etch");
    fs::write(
        &main,
        r#"
        display(name: "A B", steps: 1, delta_time: 1, traces: { x: () -> 1 });
        display(name: "a-b", steps: 1, delta_time: 1, traces: { x: () -> 2 });
        display(name: "a-b-2", steps: 1, delta_time: 1, traces: { x: () -> 3 });
    "#,
    )
    .unwrap();
    let output_dir = directory.0.join("graphs");
    execute(Cli {
        command: Command::Display {
            file: main,
            output_dir: output_dir.clone(),
        },
    })
    .unwrap();
    assert_eq!(fs::read_dir(&output_dir).unwrap().count(), 3);
    assert!(
        fs::read_to_string(output_dir.join("a-b.svg"))
            .unwrap()
            .contains("A B")
    );
    assert!(
        fs::read_to_string(output_dir.join("a-b-2.svg"))
            .unwrap()
            .contains(">a-b</text>")
    );
}

#[test]
fn runs_main_files_without_an_etch_extension() {
    let directory = TempDirectory::new();
    let main = directory.0.join("circuit");
    fs::write(&main, "42").unwrap();
    assert_eq!(
        execute(Cli {
            command: Command::Run { file: main }
        })
        .unwrap(),
        ["[0] 42"]
    );
}

#[test]
fn check_reads_only_reachable_imports_and_keeps_diagnostics() {
    let directory = TempDirectory::new();
    let main = directory.0.join("main.etch");
    fs::write(&main, "assert(condition: false)").unwrap();
    fs::write(directory.0.join("broken.etch"), [255, 254]).unwrap();
    assert!(
        execute(Cli {
            command: Command::Check { file: main.clone() }
        })
        .is_ok()
    );
    fs::write(&main, "from \"broken.etch\" import { x }").unwrap();
    assert!(
        execute(Cli {
            command: Command::Check { file: main.clone() }
        })
        .unwrap_err()
        .to_string()
        .contains("broken.etch")
    );
    fs::write(&main, "let x = unknown").unwrap();
    let diagnostic = execute(Cli {
        command: Command::Check { file: main },
    })
    .unwrap_err()
    .to_string();
    assert!(diagnostic.contains("undefined name `unknown`"));
    assert!(diagnostic.contains("8..15"));
}
