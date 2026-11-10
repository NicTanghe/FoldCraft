//! CPU-only template arrangement publication; does not step the simulation.
use clap::Parser;
use marvelous_cloth::{garment::GarmentTemplate, usd_scene::garment_stage_usda};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(about = "Publish a template's initial arrangement as a static USDA scene (CPU only)")]
struct Args {
    /// Existing ClothCraft template JSON, not a fitting-project JSON.
    template: PathBuf,
    /// Output .usda; validation finishes before an existing file is replaced.
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.output.extension().and_then(|v| v.to_str()) != Some("usda") {
        return Err("Output must have the .usda extension".into());
    }
    let template = GarmentTemplate::load(&args.template)?;
    let garment = template.compile()?;
    let usda = garment_stage_usda(&garment, &garment.initial_state)?;
    fs::write(&args.output, usda)?;
    println!(
        "Published {} panels to {}",
        garment.panels.len(),
        args.output.display()
    );
    Ok(())
}
