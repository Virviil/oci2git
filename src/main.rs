use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use oci2git::{DockerSource, ImageProcessor, NerdctlSource, Notifier, TarSource};

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

    // Create notifier with verbosity level
    let notifier = Notifier::new(cli.verbose);

    notifier.debug(&format!("Output directory: {}", cli.output.display()));
    notifier.debug(&format!("Engine: {:?}", cli.engine));
    notifier.debug(&format!(
        "Beautiful progress: {}",
        notifier.use_beautiful_progress()
    ));

    match cli.engine {
        Engine::Docker => {
            notifier.info(&format!(
                "Starting oci2git with Docker engine, image: {}",
                cli.image
            ));
            notifier.debug("Initializing Docker source");

            let source = DockerSource::new()
                .map_err(|e| anyhow!("Failed to initialize Docker source: {e}"))?;

            let processor = ImageProcessor::new(source, notifier);
            processor.convert(&cli.image, &cli.output)?;
        }
        Engine::Nerdctl => {
            notifier.info(&format!(
                "Starting oci2git with nerdctl engine, image: {}",
                cli.image
            ));
            notifier.debug("Initializing nerdctl source");

            let source = NerdctlSource::new()
                .map_err(|e| anyhow!("Failed to initialize nerdctl source: {e}"))?;

            let processor = ImageProcessor::new(source, notifier);
            processor.convert(&cli.image, &cli.output)?;
        }
        Engine::Tar => {
            notifier.info(&format!(
                "Starting oci2git with tar engine, tarball: {}",
                cli.image
            ));
            notifier.debug("Initializing tar source");

            let source =
                TarSource::new().map_err(|e| anyhow!("Failed to initialize tar source: {e}"))?;

            let processor = ImageProcessor::new(source, notifier);
            processor.convert(&cli.image, &cli.output)?;
        }
    }

    Ok(())
}
