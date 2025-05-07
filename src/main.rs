use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};
use env_logger::Env;
use log::{debug, info, LevelFilter};
use std::path::PathBuf;

use oci2git::{DockerSource, ImageProcessor, NerdctlSource, TarSource};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Engine {
    Docker,
    Nerdctl,
    Tar,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
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
        help = "Verbose mode (-v for info, -vv for debug, -vvv for trace). Also switches to text-based progress"
    )]
    verbose: u8,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging based on verbosity level
    let log_level = match cli.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    env_logger::Builder::from_env(Env::default())
        .filter_level(log_level)
        .init();

    // Determine if we should use beautiful progress indicators
    let use_beautiful_progress = cli.verbose == 0;

    debug!("Output directory: {}", cli.output.display());
    debug!("Engine: {:?}", cli.engine);
    debug!("Beautiful progress: {}", use_beautiful_progress);

    match cli.engine {
        Engine::Docker => {
            info!("Starting oci2git with Docker engine, image: {}", cli.image);
            debug!("Initializing Docker source");

            let source = DockerSource::new()
                .map_err(|e| anyhow!("Failed to initialize Docker source: {}", e))?;

            let processor = ImageProcessor::new(source);
            processor.convert(&cli.image, &cli.output, use_beautiful_progress)?;
        }
        Engine::Nerdctl => {
            info!("Starting oci2git with nerdctl engine, image: {}", cli.image);
            debug!("Initializing nerdctl source");

            let source = NerdctlSource::new()
                .map_err(|e| anyhow!("Failed to initialize nerdctl source: {}", e))?;

            let processor = ImageProcessor::new(source);
            processor.convert(&cli.image, &cli.output, use_beautiful_progress)?;
        }
        Engine::Tar => {
            info!("Starting oci2git with tar engine, tarball: {}", cli.image);
            debug!("Initializing tar source");

            let source =
                TarSource::new().map_err(|e| anyhow!("Failed to initialize tar source: {}", e))?;

            let processor = ImageProcessor::new(source);
            processor.convert(&cli.image, &cli.output, use_beautiful_progress)?;
        }
    }

    Ok(())
}
