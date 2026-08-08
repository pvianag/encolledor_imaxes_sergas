//! Local helper to verify a real (gitignored) Sergas ZIP.
//!
//! ```bash
//! cargo run --bin verify_sample -- ./10093129947808.zip
//! ```

use sergas_zip_shrinker::zip_ops::{
    analyze_zip, format_bytes, output_path_for, shrink_zip,
};
use std::env;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn main() {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("10093129947808.zip"));

    if !path.exists() {
        eprintln!("Sample not found: {}", path.display());
        eprintln!("Pass a local ZIP path (do not commit clinical samples).");
        std::process::exit(2);
    }

    println!("Analyzing {} …", path.display());
    let analysis = analyze_zip(&path);
    if let Some(err) = &analysis.error {
        eprintln!("Analysis error: {err}");
        if err != "no_dicom" {
            std::process::exit(1);
        }
    }

    println!("Input:          {}", format_bytes(analysis.input_size));
    println!("Keep / drop:    {} / {}", analysis.keep_count, analysis.drop_count);
    println!("Est. output:    {}", format_bytes(analysis.estimated_output));
    println!(
        "Est. saved:     {} ({:.1}%)",
        format_bytes(analysis.estimated_saved),
        if analysis.input_size > 0 {
            analysis.estimated_saved as f64 / analysis.input_size as f64 * 100.0
        } else {
            0.0
        }
    );

    if !analysis.is_processable() {
        std::process::exit(1);
    }

    let out = output_path_for(&path);
    println!("Writing {} …", out.display());
    let cancel = Arc::new(AtomicBool::new(false));
    let result = shrink_zip(&analysis, &cancel, &|p| {
        eprint!("\r  progress {:.0}%", p * 100.0);
    })
    .expect("shrink failed");
    eprintln!();

    let pct = if result.input_size > 0 {
        (result.input_size - result.output_size) as f64 / result.input_size as f64 * 100.0
    } else {
        0.0
    };
    println!("Output:         {}", format_bytes(result.output_size));
    println!(
        "Saved:          {} ({:.1}%)",
        format_bytes(result.input_size - result.output_size),
        pct
    );
    println!("Kept / removed: {} / {}", result.kept, result.removed);
    println!("OK — open the reduced ZIP in Weasis to confirm.");
}
