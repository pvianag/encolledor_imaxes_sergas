use anyhow::{anyhow, bail, Context, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use zip::read::ZipArchive;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Progress callback: (current_file_index, total_files, fraction_within_file 0..1, status_name)
pub type ProgressFn = Box<dyn Fn(usize, usize, f32, &str) + Send>;

#[derive(Debug, Clone)]
pub struct EntryPlan {
    pub name: String,
    pub compressed_size: u64,
    #[allow(dead_code)]
    pub uncompressed_size: u64,
    pub keep: bool,
}

#[derive(Debug, Clone)]
pub struct ZipAnalysis {
    pub path: PathBuf,
    pub input_size: u64,
    pub entries: Vec<EntryPlan>,
    pub keep_count: usize,
    pub drop_count: usize,
    pub estimated_output: u64,
    pub estimated_saved: u64,
    pub error: Option<String>,
}

impl ZipAnalysis {
    pub fn is_processable(&self) -> bool {
        self.error.is_none() && self.keep_count > 0
    }
}

#[derive(Debug, Clone)]
pub struct ShrinkResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub input_size: u64,
    pub output_size: u64,
    pub kept: usize,
    pub removed: usize,
}

pub fn output_path_for(input: &Path) -> PathBuf {
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    parent.join(format!("{stem}_reduced.zip"))
}

fn normalize_name(name: &str) -> String {
    name.replace('\\', "/")
}

fn file_name_only(normalized: &str) -> &str {
    normalized
        .rsplit('/')
        .next()
        .unwrap_or(normalized)
}

fn is_named_keep(normalized: &str) -> bool {
    let base = file_name_only(normalized);
    base.eq_ignore_ascii_case("DICOMDIR") || base.eq_ignore_ascii_case("DicomDir.inf")
}

fn looks_like_dicom(bytes: &[u8]) -> bool {
    if bytes.len() >= 132 && &bytes[128..132] == b"DICM" {
        return true;
    }
    // Some DICOM files omit the 128-byte preamble; look for a common group length tag.
    bytes.len() >= 8 && bytes[0] == 0x08 && bytes[1] == 0x00
}

fn estimate_zip_overhead(keep: &[&EntryPlan]) -> u64 {
    let mut overhead = 22u64; // EOCD
    for e in keep {
        let name_len = normalize_name(&e.name).len() as u64;
        overhead = overhead
            .saturating_add(30) // local header
            .saturating_add(46) // central header
            .saturating_add(name_len.saturating_mul(2));
    }
    overhead
}

pub fn analyze_zip(path: &Path) -> ZipAnalysis {
    let input_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            return ZipAnalysis {
                path: path.to_path_buf(),
                input_size,
                entries: vec![],
                keep_count: 0,
                drop_count: 0,
                estimated_output: 0,
                estimated_saved: 0,
                error: Some(e.to_string()),
            };
        }
    };

    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            return ZipAnalysis {
                path: path.to_path_buf(),
                input_size,
                entries: vec![],
                keep_count: 0,
                drop_count: 0,
                estimated_output: 0,
                estimated_saved: 0,
                error: Some(e.to_string()),
            };
        }
    };

    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let mut zf = match archive.by_index(i) {
            Ok(z) => z,
            Err(_) => continue,
        };
        if zf.is_dir() {
            continue;
        }
        let name = zf.name().to_string();
        let normalized = normalize_name(&name);
        let compressed_size = zf.compressed_size();
        let uncompressed_size = zf.size();

        let keep = if is_named_keep(&normalized) {
            true
        } else {
            let mut buf = [0u8; 132];
            let n = zf.read(&mut buf).unwrap_or(0);
            looks_like_dicom(&buf[..n])
        };

        entries.push(EntryPlan {
            name,
            compressed_size,
            uncompressed_size,
            keep,
        });
    }

    let keep_refs: Vec<&EntryPlan> = entries.iter().filter(|e| e.keep).collect();
    let keep_count = keep_refs.len();
    let drop_count = entries.len().saturating_sub(keep_count);
    let payload: u64 = keep_refs.iter().map(|e| e.compressed_size).sum();
    let estimated_output = payload.saturating_add(estimate_zip_overhead(&keep_refs));
    let estimated_saved = input_size.saturating_sub(estimated_output);
    let error = if keep_count == 0 {
        Some("no_dicom".to_string())
    } else {
        None
    };

    ZipAnalysis {
        path: path.to_path_buf(),
        input_size,
        entries,
        keep_count,
        drop_count,
        estimated_output,
        estimated_saved,
        error,
    }
}

pub fn shrink_zip(
    analysis: &ZipAnalysis,
    cancel: &Arc<AtomicBool>,
    on_progress: &dyn Fn(f32),
) -> Result<ShrinkResult> {
    if !analysis.is_processable() {
        bail!("ZIP has no DICOM content to keep");
    }

    let input = &analysis.path;
    let output = output_path_for(input);
    let partial = output.with_extension("zip.partial");

    if partial.exists() {
        fs::remove_file(&partial).ok();
    }

    let keep_names: Vec<String> = analysis
        .entries
        .iter()
        .filter(|e| e.keep)
        .map(|e| e.name.clone())
        .collect();
    let total_keep = keep_names.len().max(1);

    let in_file = File::open(input).with_context(|| format!("open {}", input.display()))?;
    let mut archive = ZipArchive::new(in_file).context("open zip archive")?;

    let out_file =
        File::create(&partial).with_context(|| format!("create {}", partial.display()))?;
    let mut writer = ZipWriter::new(out_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut kept = 0usize;
    for (idx, name) in keep_names.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            drop(writer);
            fs::remove_file(&partial).ok();
            bail!("cancelled");
        }

        let mut zf = archive
            .by_name(name)
            .with_context(|| format!("read entry {name}"))?;
        let out_name = normalize_name(name);
        writer
            .start_file(out_name, options)
            .with_context(|| format!("start output entry {name}"))?;

        let mut buffer = [0u8; 64 * 1024];
        loop {
            if cancel.load(Ordering::Relaxed) {
                drop(writer);
                fs::remove_file(&partial).ok();
                bail!("cancelled");
            }
            let n = zf.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            writer.write_all(&buffer[..n])?;
        }
        kept += 1;
        on_progress((idx + 1) as f32 / total_keep as f32);
    }

    writer.finish().context("finalize zip")?;

    if output.exists() {
        fs::remove_file(&output)
            .with_context(|| format!("remove existing {}", output.display()))?;
    }
    fs::rename(&partial, &output)
        .with_context(|| format!("rename {} -> {}", partial.display(), output.display()))?;

    let output_size = fs::metadata(&output)?.len();
    Ok(ShrinkResult {
        input: input.clone(),
        output,
        input_size: analysis.input_size,
        output_size,
        kept,
        removed: analysis.drop_count,
    })
}

pub fn shrink_many(
    analyses: &[ZipAnalysis],
    cancel: Arc<AtomicBool>,
    progress: ProgressFn,
) -> Result<Vec<ShrinkResult>> {
    let total = analyses.len();
    let mut results = Vec::new();
    for (i, analysis) in analyses.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            bail!("cancelled");
        }
        if !analysis.is_processable() {
            return Err(anyhow!(
                "{}: {}",
                analysis.path.display(),
                analysis.error.clone().unwrap_or_else(|| "invalid".into())
            ));
        }
        let name = analysis
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("zip");
        progress(i, total, 0.0, name);
        let result = shrink_zip(analysis, &cancel, &|frac| {
            progress(i, total, frac, name);
        })?;
        progress(i, total, 1.0, name);
        results.push(result);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn tiny_dicom() -> Vec<u8> {
        let mut v = vec![0u8; 132];
        v[128..132].copy_from_slice(b"DICM");
        v
    }

    #[test]
    fn detects_dicom_and_estimates() {
        let dir = std::env::temp_dir().join("sergas_shrink_test");
        let _ = fs::create_dir_all(&dir);
        let input = dir.join("sample_in.zip");
        {
            let f = File::create(&input).unwrap();
            let mut w = ZipWriter::new(f);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            w.start_file("Alma3DLiteCD/bin/junk.dll", opts).unwrap();
            w.write_all(b"not dicom").unwrap();
            w.start_file("FILESET/0/0/0/00000000", opts).unwrap();
            w.write_all(&tiny_dicom()).unwrap();
            w.start_file("DICOMDIR", opts).unwrap();
            w.write_all(&tiny_dicom()).unwrap();
            w.start_file("property.xml", opts).unwrap();
            w.write_all(b"<xml/>").unwrap();
            w.finish().unwrap();
        }

        let analysis = analyze_zip(&input);
        assert!(analysis.is_processable());
        assert_eq!(analysis.keep_count, 2);
        assert!(analysis.drop_count >= 2);
        assert!(analysis.estimated_output < analysis.input_size);

        let cancel = Arc::new(AtomicBool::new(false));
        let result = shrink_zip(&analysis, &cancel, &|_| {}).unwrap();
        assert!(result.output.exists());
        assert!(result.output_size < result.input_size);

        let out_analysis = analyze_zip(&result.output);
        assert_eq!(out_analysis.keep_count, 2);
        assert_eq!(out_analysis.drop_count, 0);

        let _ = fs::remove_file(&input);
        let _ = fs::remove_file(&result.output);
    }

    #[test]
    fn output_name_uses_reduced_suffix() {
        let p = PathBuf::from("/tmp/estudo.zip");
        assert_eq!(output_path_for(&p), PathBuf::from("/tmp/estudo_reduced.zip"));
    }
}

pub fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let v = n as f64;
    if v >= GB {
        format!("{:.2} GB", v / GB)
    } else if v >= MB {
        format!("{:.2} MB", v / MB)
    } else if v >= KB {
        format!("{:.1} KB", v / KB)
    } else {
        format!("{n} B")
    }
}
