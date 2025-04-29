use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};
use env_logger::Env;
use log::{debug, info, LevelFilter};
use std::path::PathBuf;

use oci2git::{DockerEngine, ImageToGitConverter, NerdctlEngine};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Engine {
    Docker,
    Nerdctl,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(help = "Image name to convert (e.g., ubuntu:latest)")]
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
        help = "Container engine to use"
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

    info!("Starting oci2git with image: {}", cli.image);
    debug!("Output directory: {}", cli.output.display());
    debug!("Engine: {:?}", cli.engine);
    debug!("Beautiful progress: {}", use_beautiful_progress);

    match cli.engine {
        Engine::Docker => {
            debug!("Initializing Docker engine");
            let engine = DockerEngine::new()
                .map_err(|e| anyhow!("Failed to initialize Docker engine: {}", e))?;
            let converter = ImageToGitConverter::new(engine);
            converter.convert(&cli.image, &cli.output, use_beautiful_progress)?;
        }
        Engine::Nerdctl => {
            debug!("Initializing nerdctl engine");
            let engine = NerdctlEngine::new()
                .map_err(|e| anyhow!("Failed to initialize nerdctl engine: {}", e))?;
            let converter = ImageToGitConverter::new(engine);
            converter.convert(&cli.image, &cli.output, use_beautiful_progress)?;
        }
    }

    Ok(())
}
