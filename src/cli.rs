use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::checker::ltl::counterexample_as_json;
use crate::checker::{check_properties, CheckReport, CheckStatus};
use crate::diagnostics::{Result, TplError};
use crate::loader::{load_spec, LoadOptions, LoadedSpec};
use crate::model::{build_graph_with_options, BuildOptions, ModelGraph};
use crate::sema::check_source_file;
use crate::syntax::{parse_source_file, PrintMode, Printer, PropertyKind, SourceFile};

#[derive(Debug, Parser)]
#[command(name = "tplgine")]
#[command(about = "Temporal propositional logic engine and model checker")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(value_name = "SPEC")]
    spec: Option<PathBuf>,

    #[command(flatten)]
    printer: PrinterArgs,

    #[arg(long, value_enum, default_value_t = OutputFormat::Human, global = true)]
    format: OutputFormat,

    #[arg(long = "include-path", value_name = "DIR", global = true)]
    include_paths: Vec<PathBuf>,

    #[arg(long = "max-states", default_value_t = 100_000, global = true)]
    max_states: usize,

    #[arg(long = "show-trace", global = true)]
    show_trace: bool,

    #[arg(long = "dump-graph", global = true)]
    dump_graph: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    Check {
        #[arg(value_name = "SPEC")]
        spec: PathBuf,
    },
    Parse {
        #[arg(value_name = "SPEC")]
        spec: PathBuf,
    },
    Fmt {
        #[arg(value_name = "SPEC")]
        spec: PathBuf,
    },
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Args)]
struct PrinterArgs {
    #[arg(long, global = true, conflicts_with_all = ["print_ascii_operators", "print_unicode_operators"])]
    print_keywords: bool,

    #[arg(long, global = true, conflicts_with_all = ["print_keywords", "print_unicode_operators"])]
    print_ascii_operators: bool,

    #[arg(
        long,
        global = true,
        conflicts_with_all = ["print_keywords", "print_ascii_operators"]
    )]
    print_unicode_operators: bool,

    #[arg(
        long = "print-unicode-oeprators",
        hide = true,
        global = true,
        conflicts_with_all = ["print_keywords", "print_ascii_operators"]
    )]
    print_unicode_oeprators: bool,
}

impl PrinterArgs {
    fn mode(self) -> PrintMode {
        if self.print_keywords {
            PrintMode::Keywords
        } else if self.print_ascii_operators {
            PrintMode::AsciiOperators
        } else if self.print_unicode_operators || self.print_unicode_oeprators {
            PrintMode::UnicodeOperators
        } else {
            PrintMode::UnicodeOperators
        }
    }
}

pub fn run() -> ExitCode {
    let cli = Cli::parse();

    match run_cli(cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(exit_code(&err))
        }
    }
}

fn run_cli(cli: Cli) -> Result<bool> {
    let print_mode = cli.printer.mode();
    let load_options = LoadOptions {
        include_paths: cli.include_paths,
    };
    let build_options = BuildOptions {
        max_states: cli.max_states,
    };

    match cli.command {
        Some(Command::Check { spec }) => check(
            &spec,
            cli.format,
            &load_options,
            &build_options,
            cli.show_trace,
            cli.dump_graph,
        ),
        Some(Command::Parse { spec }) => {
            parse(&spec, cli.format, &load_options)?;
            Ok(true)
        }
        Some(Command::Fmt { spec }) => {
            fmt(&spec, print_mode)?;
            Ok(true)
        }
        None => {
            let spec = cli.spec.ok_or_else(|| TplError::Unsupported {
                message: "missing <spec>.tpl argument".to_owned(),
            })?;
            check(
                &spec,
                cli.format,
                &load_options,
                &build_options,
                cli.show_trace,
                cli.dump_graph,
            )
        }
    }
}

fn check(
    path: &Path,
    format: OutputFormat,
    load_options: &LoadOptions,
    build_options: &BuildOptions,
    show_trace: bool,
    dump_graph: bool,
) -> Result<bool> {
    let spec = load_spec(path, load_options)?;
    check_source_file(&spec.source)?;
    let graph = build_graph_with_options(&spec.source, build_options)?;
    let report = check_properties(&spec.source, &graph)?;

    match format {
        OutputFormat::Human => print_human_report(&spec, &graph, &report, show_trace, dump_graph),
        OutputFormat::Json => print_json_report(&spec, &graph, &report),
    }

    Ok(report.status == CheckStatus::Pass)
}

fn parse(path: &Path, format: OutputFormat, load_options: &LoadOptions) -> Result<()> {
    let spec = load_spec(path, load_options)?;

    match format {
        OutputFormat::Human => println!("{:#?}", spec.source),
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "tplgine",
                    "status": "parsed",
                    "root": spec.root,
                    "files": spec.files.len(),
                    "items": spec.source.item_count(),
                    "diagnostics": []
                })
            );
        }
    }

    Ok(())
}

fn fmt(path: &Path, print_mode: PrintMode) -> Result<()> {
    let file = load_and_parse(path)?;
    print!("{}", Printer::new(print_mode).print_source_file(&file));
    Ok(())
}

fn print_human_report(
    spec: &LoadedSpec,
    graph: &ModelGraph,
    report: &CheckReport,
    show_trace: bool,
    dump_graph: bool,
) {
    println!(
        "{} loaded {} file(s), typechecked {} item(s), built {} reachable state(s) and {} transition edge(s)",
        match report.status {
            CheckStatus::Pass => "OK",
            CheckStatus::Fail => "FAIL",
        },
        spec.files.len(),
        spec.source.item_count(),
        graph.states.len(),
        graph.edge_count()
    );

    for property in &report.properties {
        let kind_label = match property.kind {
            PropertyKind::Property => "property",
            PropertyKind::Invalid => "invalid",
        };
        println!(
            "{} {} {}",
            match property.status {
                CheckStatus::Pass => "PASS",
                CheckStatus::Fail => "FAIL",
            },
            kind_label,
            property.name
        );

        if show_trace {
            if let Some(counterexample) = &property.counterexample {
                println!("counterexample:");
                for (index, state) in counterexample.states.iter().enumerate() {
                    println!("  s{index}: {}", format_state(graph, state));
                }
                if let Some(cycle_start) = counterexample.cycle_start {
                    println!("  cycle starts at s{cycle_start}");
                }
            }
        }
    }

    if dump_graph {
        println!("reachable graph:");
        for (index, state) in graph.states.iter().enumerate() {
            let successors = graph.edges[index]
                .iter()
                .map(|successor| format!("s{successor}"))
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "  s{index}: {} -> [{}]",
                format_state(graph, state),
                successors
            );
        }
    }
}

fn print_json_report(spec: &LoadedSpec, graph: &ModelGraph, report: &CheckReport) {
    let properties = report
        .properties
        .iter()
        .map(|property| {
            let mut object = serde_json::Map::new();
            object.insert("name".to_owned(), serde_json::json!(property.name));
            object.insert("kind".to_owned(), serde_json::json!(property.kind));
            object.insert("status".to_owned(), serde_json::json!(property.status));
            if let Some(counterexample) = &property.counterexample {
                object.insert(
                    "counterexample".to_owned(),
                    counterexample_as_json(graph, counterexample),
                );
            }
            serde_json::Value::Object(object)
        })
        .collect::<Vec<_>>();

    println!(
        "{}",
        serde_json::json!({
            "tool": "tplgine",
            "status": report.status,
            "root": spec.root,
            "files": spec.files.len(),
            "items": spec.source.item_count(),
            "states": graph.states.len(),
            "transitions": graph.edge_count(),
            "properties": properties,
            "diagnostics": []
        })
    );
}

fn format_state(graph: &ModelGraph, state: &crate::model::State) -> String {
    graph
        .variables
        .iter()
        .zip(&state.values)
        .map(|(name, value)| format!("{name} = {value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn load_and_parse(path: &Path) -> Result<SourceFile> {
    validate_tpl_extension(path)?;
    let source = fs::read_to_string(path).map_err(|source| TplError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    parse_source_file(path, &source)
}

fn validate_tpl_extension(path: &Path) -> Result<()> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("tpl") {
        Ok(())
    } else {
        Err(TplError::InvalidExtension {
            path: path.to_path_buf(),
        })
    }
}

fn exit_code(err: &TplError) -> u8 {
    match err {
        TplError::Parse { .. } => 2,
        TplError::Semantic { .. } => 3,
        TplError::ReadFile { .. } | TplError::InvalidExtension { .. } | TplError::Import { .. } => {
            4
        }
        TplError::Model { .. } => 5,
        TplError::Unsupported { .. } => 6,
    }
}
