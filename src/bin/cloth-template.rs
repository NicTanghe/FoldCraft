//! Headless authoring tools; simulation defaults to WGSL when desktop is built.
use clap::{Parser, Subcommand};
use marvelous_cloth::{
    garment::{GarmentTemplate, export, production_metrics},
    solver::{ReferenceSolver, SolverSettings},
};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(about = "Validate, compile and simulate garment templates", version)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Write a small editable example. Refuses to overwrite an existing file.
    Example {
        output: PathBuf,
    },
    Validate {
        template: PathBuf,
    },
    /// Report net rest-mesh area and mass per panel and fabric (not nesting consumption).
    Metrics {
        template: PathBuf,
    },
    /// Diagnose authored sewing without running simulation. Invalid sewing exits nonzero after JSON output.
    AuditSewing {
        template: PathBuf,
        /// Read the template embedded in a fitting-project v1 JSON; ignore avatar and draped state.
        #[arg(long)]
        project: bool,
    },
    /// Produce rest OBJ, preview and measured SVG outlines, and fiber frames.
    Bake {
        template: PathBuf,
        #[arg(long, default_value = "output/template-bake")]
        output: PathBuf,
    },
    Simulate {
        template: PathBuf,
        #[arg(long, default_value = "output/template-simulation")]
        output: PathBuf,
        #[arg(long, default_value_t = 30)]
        steps: usize,
        /// Historical CPU Newton control (no GPU initialization).
        #[arg(long, alias = "reference")]
        cpu: bool,
        #[arg(long,default_value_t=1.0/60.0)]
        dt: f64,
        #[arg(long, default_value_t = 2)]
        threads: usize,
    },
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Args::parse().command {
        Command::Example { output } => {
            let t = GarmentTemplate::two_panel_demo();
            t.compile()?;
            let mut file = File::create_new(output)?;
            file.write_all(serde_json::to_string_pretty(&t)?.as_bytes())?;
        }
        Command::Validate { template } => {
            let t = GarmentTemplate::load(&template)?;
            let g = t.compile()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                "name":t.name,"panels":g.panels.len(),"vertices":g.mesh.uv.len(),"triangles":g.mesh.triangles.len(),
                "seams":g.seams.len(),"initial_gap_m":g.max_seam_gap(&g.initial_state.x),"warnings":g.warnings}))?
            );
        }
        Command::Metrics { template } => {
            let template = GarmentTemplate::load(&template)?;
            let report = production_metrics::report(&template)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::AuditSewing { template, project } => {
            // The regular loader compiles first and would hide all but the first
            // sewing error. Geometry validation remains inside the audit.
            let json = fs::read_to_string(template)?;
            let template: GarmentTemplate = if project {
                let project: marvelous_cloth::fitting::FittingProject =
                    serde_json::from_str(&json)?;
                if project.version != 1 {
                    return Err("Unsupported fitting project version".into());
                }
                project.template
            } else {
                serde_json::from_str(&json)?
            };
            let report = marvelous_cloth::garment::seam::audit::report(&template)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.valid {
                return Err("Invalid authored sewing; inspect the JSON diagnostics".into());
            }
        }
        Command::Bake { template, output } => {
            let t = GarmentTemplate::load(&template)?;
            let g = t.compile()?;
            export::write_artifacts(&t, &g, &g.initial_state, &output)?;
            println!("CPU artifacts written to {}", output.display());
        }
        Command::Simulate {
            template,
            output,
            steps,
            cpu,
            dt,
            threads,
        } => {
            if steps == 0 || !(1..=64).contains(&threads) {
                return Err("Require steps > 0 and 1..64 CPU threads".into());
            }
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build_global()?;
            let settings = SolverSettings {
                dt,
                ..Default::default()
            };
            settings.validate()?;
            let t = GarmentTemplate::load(&template)?;
            let g = t.compile()?;
            let mut state = g.initial_state.clone();
            let initial_gap = g.max_seam_gap(&state.x);
            fs::create_dir_all(&output)?;
            fs::write(output.join("initial.obj"), export::obj(&g, &state))?;
            let mut log = BufWriter::new(File::create(output.join("metrics.jsonl"))?);
            #[cfg(feature = "desktop")]
            if !cpu {
                let g = std::sync::Arc::new(g);
                let (device, queue) = marvelous_cloth::gpu_avbd::request_device()?;
                let gpu = marvelous_cloth::gpu_avbd::GpuAvbd::new(device, queue);
                let mut solver = marvelous_cloth::vbd::VbdSolver::new(g.clone(), None, &state)?;
                let simulation = marvelous_cloth::vbd::VbdSettings {
                    dt,
                    ..Default::default()
                };
                let mut last = None;
                for _ in 0..steps {
                    let m = gpu.step(&mut solver, &mut state, &simulation, None)?;
                    serde_json::to_writer(&mut log, &m)?;
                    writeln!(log)?;
                    last = Some(m);
                }
                log.flush()?;
                export::write_artifacts(&t, &g, &state, &output)?;
                let report = serde_json::json!({"backend":marvelous_cloth::gpu_avbd::BACKEND,"gpu_initialized":true,"steps":steps,"settings":simulation,"initial_seam_gap_m":initial_gap,"final_seam_gap_m":g.max_seam_gap(&state.x),"last_step":last,"residual":"unavailable; fixed iteration budget"});
                fs::write(
                    output.join("run.json"),
                    serde_json::to_string_pretty(&report)?,
                )?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                return Ok(());
            }
            #[cfg(not(feature = "desktop"))]
            let _ = cpu;
            let mut converged = 0;
            let mut last = None;
            for _ in 0..steps {
                let metrics = ReferenceSolver::step_garment(&g, &mut state, &settings, None)?;
                converged += usize::from(metrics.converged);
                serde_json::to_writer(&mut log, &metrics)?;
                writeln!(log)?;
                last = Some(metrics);
            }
            log.flush()?;
            export::write_artifacts(&t, &g, &state, &output)?;
            let report = serde_json::json!({"backend":"cpu-f64","gpu_initialized":false,"threads":threads,"solver":settings,
                "steps":steps,"converged_steps":converged,"initial_seam_gap_m":initial_gap,"final_seam_gap_m":g.max_seam_gap(&state.x),
                "last_step":last,"warnings":g.warnings});
            fs::write(
                output.join("run.json"),
                serde_json::to_string_pretty(&report)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if converged != steps {
                return Err(
                    "Some steps did not converge; inspect metrics.jsonl before using the result"
                        .into(),
                );
            }
        }
    }
    Ok(())
}
