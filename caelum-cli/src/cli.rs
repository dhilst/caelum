use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};

use caelum_kernel::checker::ltl::counterexample_as_json;
use caelum_kernel::checker::{check_properties, CheckReport, CheckStatus};
use caelum_kernel::diagnostics::{CaelumError, Result};
use caelum_kernel::loader::{load_spec_with, LoadedSpec};
use caelum_kernel::model::{build_graph_with_options, BuildOptions, ModelGraph};
use caelum_kernel::sema::check_source_file;
use caelum_kernel::syntax::{parse_source_file, PrintMode, Printer, PropertyKind, SourceFile};

use crate::fs_resolver::StdFsResolver;

#[derive(Debug, Parser)]
#[command(name = "caelum")]
#[command(about = "LTL model checker")]
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

    #[arg(long, value_enum, default_value_t = Engine::Explicit, global = true)]
    engine: Engine,

    #[arg(long, value_enum, default_value_t = SolverChoice::Z3, global = true)]
    solver: SolverChoice,

    #[arg(long = "bmc-depth", default_value_t = 50, global = true)]
    bmc_depth: usize,

    /// Try k-induction on safety properties that pass the base case so we
    /// can certify them as invariants rather than just "no counterexample
    /// within k steps". Only meaningful with `--engine bmc`.
    #[arg(long = "prove", alias = "k-induction", global = true)]
    prove: bool,
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

#[derive(Debug, Copy, Clone, ValueEnum, PartialEq, Eq)]
enum Engine {
    Explicit,
    Bmc,
}

#[derive(Debug, Copy, Clone, ValueEnum, PartialEq, Eq)]
enum SolverChoice {
    Varisat,
    Cadical,
    Z3,
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
    let resolver = StdFsResolver::new(cli.include_paths);
    let build_options = BuildOptions {
        max_states: cli.max_states,
    };

    let engine_opts = EngineOptions {
        engine: cli.engine,
        solver: cli.solver,
        bmc_depth: cli.bmc_depth,
        prove: cli.prove,
    };

    match cli.command {
        Some(Command::Check { spec }) => check(
            &spec,
            cli.format,
            &resolver,
            &build_options,
            &engine_opts,
            cli.show_trace,
            cli.dump_graph,
        ),
        Some(Command::Parse { spec }) => {
            parse(&spec, cli.format, &resolver)?;
            Ok(true)
        }
        Some(Command::Fmt { spec }) => {
            fmt(&spec, print_mode)?;
            Ok(true)
        }
        None => {
            let spec = cli.spec.ok_or_else(|| CaelumError::Unsupported {
                message: "missing <spec>.lum argument".to_owned(),
            })?;
            check(
                &spec,
                cli.format,
                &resolver,
                &build_options,
                &engine_opts,
                cli.show_trace,
                cli.dump_graph,
            )
        }
    }
}

#[derive(Debug, Clone)]
struct EngineOptions {
    engine: Engine,
    solver: SolverChoice,
    bmc_depth: usize,
    prove: bool,
}

fn check(
    path: &Path,
    format: OutputFormat,
    resolver: &StdFsResolver,
    build_options: &BuildOptions,
    engine: &EngineOptions,
    show_trace: bool,
    dump_graph: bool,
) -> Result<bool> {
    let root = StdFsResolver::canonical_id(path)?;
    let spec = load_spec_with(&root, resolver)?;
    check_source_file(&spec.source)?;

    match engine.engine {
        Engine::Explicit => {
            let graph = build_graph_with_options(&spec.source, build_options)?;
            let report = check_properties(&spec.source, &graph)?;

            match format {
                OutputFormat::Human => {
                    print_human_report(&spec, Some(&graph), &report, show_trace, dump_graph)
                }
                OutputFormat::Json => print_json_report(&spec, Some(&graph), &report),
            }

            Ok(report.status == CheckStatus::Pass)
        }
        Engine::Bmc => check_bmc(&spec, format, engine, show_trace),
    }
}

#[cfg(any(feature = "bmc-varisat", feature = "bmc-cadical", feature = "bmc-z3"))]
fn check_bmc(
    spec: &LoadedSpec,
    format: OutputFormat,
    engine: &EngineOptions,
    show_trace: bool,
) -> Result<bool> {
    use caelum_kernel::bmc::{check_with_bmc, BmcOptions, SolverBackend};
    let backend = match engine.solver {
        #[cfg(feature = "bmc-varisat")]
        SolverChoice::Varisat => SolverBackend::Varisat,
        #[cfg(not(feature = "bmc-varisat"))]
        SolverChoice::Varisat => {
            return Err(CaelumError::Unsupported {
                message: "varisat backend not compiled in (enable feature `bmc-varisat`)".into(),
            })
        }
        #[cfg(feature = "bmc-cadical")]
        SolverChoice::Cadical => SolverBackend::Cadical,
        #[cfg(not(feature = "bmc-cadical"))]
        SolverChoice::Cadical => {
            return Err(CaelumError::Unsupported {
                message: "cadical backend not compiled in (enable feature `bmc-cadical`)".into(),
            })
        }
        #[cfg(feature = "bmc-z3")]
        SolverChoice::Z3 => SolverBackend::Z3,
        #[cfg(not(feature = "bmc-z3"))]
        SolverChoice::Z3 => {
            return Err(CaelumError::Unsupported {
                message: "z3 backend not compiled in (enable feature `bmc-z3`)".into(),
            })
        }
    };

    let opts = BmcOptions {
        depth: engine.bmc_depth,
        prove: engine.prove,
    };
    let report = check_with_bmc(&spec.source, &opts, backend)?;
    match format {
        OutputFormat::Human => print_human_report(spec, None, &report, show_trace, false),
        OutputFormat::Json => print_json_report(spec, None, &report),
    }
    Ok(report.status != CheckStatus::Fail)
}

#[cfg(not(any(feature = "bmc-varisat", feature = "bmc-cadical", feature = "bmc-z3")))]
fn check_bmc(
    _spec: &LoadedSpec,
    _format: OutputFormat,
    _engine: &EngineOptions,
    _show_trace: bool,
) -> Result<bool> {
    Err(CaelumError::Unsupported {
        message:
            "BMC engine not compiled in; rebuild with --features bmc-varisat, bmc-cadical or bmc-z3"
                .into(),
    })
}

fn parse(path: &Path, format: OutputFormat, resolver: &StdFsResolver) -> Result<()> {
    let root = StdFsResolver::canonical_id(path)?;
    let spec = load_spec_with(&root, resolver)?;

    match format {
        OutputFormat::Human => println!("{:#?}", spec.source),
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "caelum",
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
    graph: Option<&ModelGraph>,
    report: &CheckReport,
    show_trace: bool,
    dump_graph: bool,
) {
    let header_status = match report.status {
        CheckStatus::Pass => "OK",
        CheckStatus::Fail => "FAIL",
        CheckStatus::Skipped => "SKIP",
        CheckStatus::Certified => "OK",
    };
    if let Some(graph) = graph {
        println!(
            "{header_status} loaded {} file(s), typechecked {} item(s), built {} reachable state(s) and {} transition edge(s)",
            spec.files.len(),
            spec.source.item_count(),
            graph.states.len(),
            graph.edge_count()
        );
    } else {
        println!(
            "{header_status} loaded {} file(s), typechecked {} item(s), engine=bmc",
            spec.files.len(),
            spec.source.item_count(),
        );
    }

    let var_names = bmc_variable_names(spec, graph);

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
                CheckStatus::Skipped => "SKIP",
                CheckStatus::Certified => "CERT",
            },
            kind_label,
            property.name
        );

        if let Some(note) = &property.note {
            println!("  note: {note}");
        }

        if show_trace {
            if let Some(counterexample) = &property.counterexample {
                println!("counterexample:");
                for (index, state) in counterexample.states.iter().enumerate() {
                    println!("  s{index}: {}", format_state_named(&var_names, state));
                }
                if let Some(cycle_start) = counterexample.cycle_start {
                    println!("  cycle starts at s{cycle_start}");
                }
            }
        }
    }

    if dump_graph {
        if let Some(graph) = graph {
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
}

fn print_json_report(spec: &LoadedSpec, graph: Option<&ModelGraph>, report: &CheckReport) {
    let var_names = bmc_variable_names(spec, graph);
    let properties = report
        .properties
        .iter()
        .map(|property| {
            let mut object = serde_json::Map::new();
            object.insert("name".to_owned(), serde_json::json!(property.name));
            object.insert("kind".to_owned(), serde_json::json!(property.kind));
            object.insert("status".to_owned(), serde_json::json!(property.status));
            if let Some(note) = &property.note {
                object.insert("note".to_owned(), serde_json::json!(note));
            }
            if let Some(counterexample) = &property.counterexample {
                if let Some(graph) = graph {
                    object.insert(
                        "counterexample".to_owned(),
                        counterexample_as_json(graph, counterexample),
                    );
                } else {
                    object.insert(
                        "counterexample".to_owned(),
                        counterexample_to_json_named(&var_names, counterexample),
                    );
                }
            }
            serde_json::Value::Object(object)
        })
        .collect::<Vec<_>>();

    let mut top = serde_json::Map::new();
    top.insert("tool".into(), serde_json::json!("caelum"));
    top.insert("status".into(), serde_json::json!(report.status));
    top.insert("root".into(), serde_json::json!(spec.root));
    top.insert("files".into(), serde_json::json!(spec.files.len()));
    top.insert("items".into(), serde_json::json!(spec.source.item_count()));
    if let Some(graph) = graph {
        top.insert("states".into(), serde_json::json!(graph.states.len()));
        top.insert("transitions".into(), serde_json::json!(graph.edge_count()));
    } else {
        top.insert("engine".into(), serde_json::json!("bmc"));
    }
    top.insert("properties".into(), serde_json::Value::Array(properties));
    top.insert("diagnostics".into(), serde_json::json!([]));
    println!("{}", serde_json::Value::Object(top));
}

fn format_state(graph: &ModelGraph, state: &caelum_kernel::model::State) -> String {
    graph
        .variables
        .iter()
        .zip(&state.values)
        .map(|(name, value)| format!("{name} = {value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_state_named(names: &[String], state: &caelum_kernel::model::State) -> String {
    names
        .iter()
        .zip(&state.values)
        .map(|(name, value)| format!("{name} = {value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn bmc_variable_names(spec: &LoadedSpec, graph: Option<&ModelGraph>) -> Vec<String> {
    if let Some(graph) = graph {
        return graph.variables.clone();
    }
    spec.source
        .items
        .iter()
        .filter_map(|item| match item {
            caelum_kernel::syntax::Item::Var(decl) => Some(decl.name.clone()),
            _ => None,
        })
        .collect()
}

fn counterexample_to_json_named(
    names: &[String],
    counterexample: &caelum_kernel::checker::Counterexample,
) -> serde_json::Value {
    let states = counterexample
        .states
        .iter()
        .map(|state| {
            let mut obj = serde_json::Map::new();
            for (name, value) in names.iter().zip(&state.values) {
                obj.insert(name.clone(), serde_json::json!(value));
            }
            serde_json::Value::Object(obj)
        })
        .collect::<Vec<_>>();
    let mut object = serde_json::Map::new();
    object.insert("states".into(), serde_json::Value::Array(states));
    if let Some(cycle_start) = counterexample.cycle_start {
        object.insert("cycle_start".into(), serde_json::json!(cycle_start));
    }
    serde_json::Value::Object(object)
}

fn load_and_parse(path: &Path) -> Result<SourceFile> {
    validate_extension(path)?;
    let source = fs::read_to_string(path).map_err(|source| CaelumError::ReadFile {
        path: path.display().to_string(),
        message: source.to_string(),
    })?;
    parse_source_file(path, &source)
}

fn validate_extension(path: &Path) -> Result<()> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("lum") {
        Ok(())
    } else {
        Err(CaelumError::InvalidExtension {
            path: path.display().to_string(),
        })
    }
}

fn exit_code(err: &CaelumError) -> u8 {
    match err {
        CaelumError::Parse { .. } => 2,
        CaelumError::Semantic { .. } => 3,
        CaelumError::ReadFile { .. }
        | CaelumError::InvalidExtension { .. }
        | CaelumError::Import { .. } => 4,
        CaelumError::Model { .. } => 5,
        CaelumError::Unsupported { .. } => 6,
    }
}
