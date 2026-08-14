use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use okf_attestor_core::{BundleReport, ConceptReport, Verifier};

#[derive(Debug, Parser)]
#[command(name = "okf-attestor", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Re-execute Attested Computations in a local OKF bundle.
    Verify {
        /// Local OKF bundle directory.
        bundle_path: PathBuf,
        /// Verify only this bundle-relative concept id.
        #[arg(long)]
        concept: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(3)
        }
    }
}

fn run(cli: Cli) -> Result<u8, Box<dyn std::error::Error>> {
    match cli.command {
        Command::Verify {
            bundle_path,
            concept,
            format,
        } => {
            let verifier = Verifier::new()?;
            if let Some(concept) = concept {
                let result = verifier.verify_concept(&bundle_path, &concept)?;
                print_single(&bundle_path.display().to_string(), result, format)
            } else {
                let report = verifier.verify_bundle(&bundle_path)?;
                let code = report.exit_code();
                print_bundle(&report, format)?;
                Ok(code)
            }
        }
    }
}

fn print_single(
    bundle: &str,
    result: ConceptReport,
    format: OutputFormat,
) -> Result<u8, Box<dyn std::error::Error>> {
    let report = BundleReport {
        schema_version: okf_attestor_core::REPORT_SCHEMA_VERSION,
        bundle: bundle.to_owned(),
        results: vec![result],
    };
    let code = report.exit_code();
    print_bundle(&report, format)?;
    Ok(code)
}

fn print_bundle(
    report: &BundleReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
        OutputFormat::Human => {
            if report.results.is_empty() {
                println!("No Attested Computations found in {}", report.bundle);
            }
            for result in &report.results {
                println!(
                    "{}: {:?} ({:?}) — {}",
                    result.concept, result.verdict, result.reason_code, result.message
                );
                if let Some(difference) = &result.difference {
                    println!("  expected sha256: {}", difference.expected_sha256);
                    println!("  actual sha256:   {}", difference.actual_sha256);
                    if let Some(expected) = &difference.expected_utf8 {
                        println!("  expected: {expected:?}");
                    }
                    if let Some(actual) = &difference.actual_utf8 {
                        println!("  actual:   {actual:?}");
                    }
                }
            }
        }
    }
    Ok(())
}
