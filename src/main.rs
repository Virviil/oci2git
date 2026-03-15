use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};

use oci2git::{DockerSource, ImageProcessor, NerdctlSource, Notifier, TarSource};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Engine {
    Docker,
    Nerdctl,
    Tar,
}

#[derive(Debug, Args)]
struct ConvertArgs {
    #[arg(
        help = "Image name to convert (e.g., ubuntu:latest) or path to tarball when using tar engine"
    )]
    image: String,

    #[arg(
        short,
        long,
        default_value = "./container_repo",
        help = "Output directory for Git repository"
    )]
    output: PathBuf,

    #[arg(
        short,
        long,
        value_enum,
        default_value = "docker",
        help = "Container engine to use (docker, nerdctl, tar)"
    )]
    engine: Engine,

    #[arg(
        short,
        long,
        action = clap::ArgAction::Count,
        help = "Verbose mode (-v for info, -vv for debug, -vvv for trace)"
    )]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert an OCI/Docker image to a Git repository
    Convert(ConvertArgs),

    /// Generate a YAML filesystem bill of materials
    Fsbom {
        #[arg(
            help = "Image name (e.g., ubuntu:latest) or path to tarball when using tar engine"
        )]
        image: String,

        #[arg(
            short,
            long,
            default_value = "./fsbom.yml",
            help = "Output path for the YAML BOM file"
        )]
        output: PathBuf,

        #[arg(
            short,
            long,
            value_enum,
            default_value = "docker",
            help = "Container engine to use (docker, nerdctl, tar)"
        )]
        engine: Engine,

        #[arg(
            short,
            long,
            action = clap::ArgAction::Count,
            help = "Verbose mode (-v for info, -vv for debug, -vvv for trace)"
        )]
        verbose: u8,
    },
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        help = "Image name to convert (e.g., ubuntu:latest) or path to tarball when using tar engine"
    )]
    image: Option<String>,

    #[arg(
        short,
        long,
        default_value = "./container_repo",
        help = "Output directory for Git repository"
    )]
    output: PathBuf,

    #[arg(
        short,
        long,
        value_enum,
        default_value = "docker",
        help = "Container engine to use (docker, nerdctl, tar)"
    )]
    engine: Engine,

    #[arg(
        short,
        long,
        action = clap::ArgAction::Count,
        help = "Verbose mode (-v for info, -vv for debug, -vvv for trace)"
    )]
    verbose: u8,
}

fn run_convert(image: &str, output: &Path, engine: Engine, verbose: u8) -> Result<()> {
    let notifier = Notifier::new(verbose);
    notifier.debug(&format!("Output directory: {}", output.display()));
    notifier.debug(&format!("Engine: {engine:?}"));

    match engine {
        Engine::Docker => {
            notifier.info(&format!(
                "Starting oci2git with Docker engine, image: {image}"
            ));
            let source = DockerSource::new()
                .map_err(|e| anyhow!("Failed to initialize Docker source: {e}"))?;
            ImageProcessor::new(source, notifier).convert(image, output)?;
        }
        Engine::Nerdctl => {
            notifier.info(&format!(
                "Starting oci2git with nerdctl engine, image: {image}"
            ));
            let source = NerdctlSource::new()
                .map_err(|e| anyhow!("Failed to initialize nerdctl source: {e}"))?;
            ImageProcessor::new(source, notifier).convert(image, output)?;
        }
        Engine::Tar => {
            notifier.info(&format!(
                "Starting oci2git with tar engine, tarball: {image}"
            ));
            let source =
                TarSource::new().map_err(|e| anyhow!("Failed to initialize tar source: {e}"))?;
            ImageProcessor::new(source, notifier).convert(image, output)?;
        }
    }
    Ok(())
}

fn run_fsbom(image: &str, output: &Path, engine: Engine, verbose: u8) -> Result<()> {
    let notifier = Notifier::new(verbose);
    notifier.debug(&format!("Output path: {}", output.display()));
    notifier.debug(&format!("Engine: {engine:?}"));

    match engine {
        Engine::Docker => {
            notifier.info(&format!(
                "Generating fsbom with Docker engine, image: {image}"
            ));
            let source = DockerSource::new()
                .map_err(|e| anyhow!("Failed to initialize Docker source: {e}"))?;
            ImageProcessor::new(source, notifier).generate_fsbom(image, output)?;
        }
        Engine::Nerdctl => {
            notifier.info(&format!(
                "Generating fsbom with nerdctl engine, image: {image}"
            ));
            let source = NerdctlSource::new()
                .map_err(|e| anyhow!("Failed to initialize nerdctl source: {e}"))?;
            ImageProcessor::new(source, notifier).generate_fsbom(image, output)?;
        }
        Engine::Tar => {
            notifier.info(&format!(
                "Generating fsbom with tar engine, tarball: {image}"
            ));
            let source =
                TarSource::new().map_err(|e| anyhow!("Failed to initialize tar source: {e}"))?;
            ImageProcessor::new(source, notifier).generate_fsbom(image, output)?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let cmd = cli.command.unwrap_or_else(|| {
        Commands::Convert(ConvertArgs {
            image: cli.image.unwrap_or_default(),
            output: cli.output,
            engine: cli.engine,
            verbose: cli.verbose,
        })
    });

    match cmd {
        Commands::Convert(args) => {
            if args.image.is_empty() {
                return Err(anyhow!(
                    "Image name required.\nUsage: oci2git convert <IMAGE>"
                ));
            }
            run_convert(&args.image, &args.output, args.engine, args.verbose)?;
        }
        Commands::Fsbom {
            image,
            output,
            engine,
            verbose,
        } => {
            run_fsbom(&image, &output, engine, verbose)?;
        }
    }

    Ok(())
}
