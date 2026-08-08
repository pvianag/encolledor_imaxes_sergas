use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/app_icon.ico");
    println!("cargo:rerun-if-changed=assets/app_icon.png");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    emit_git_info();
    embed_windows_icon();
}

fn emit_git_info() {
    let commit = git_out(["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let commit_short =
        git_out(["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let tag = git_out(["describe", "--tags", "--exact-match", "HEAD"])
        .or_else(|| git_out(["describe", "--tags", "--abbrev=0"]))
        .unwrap_or_else(|| format!("v{}", env!("CARGO_PKG_VERSION")));
    let describe = git_out(["describe", "--tags", "--always", "--dirty"])
        .unwrap_or_else(|| commit_short.clone());

    // Prefer CI-provided values on tagged GitHub Actions builds.
    let tag = if std::env::var("GITHUB_REF_TYPE").as_deref() == Ok("tag") {
        std::env::var("GITHUB_REF_NAME").unwrap_or(tag)
    } else {
        tag
    };
    let commit = std::env::var("GITHUB_SHA").unwrap_or(commit);
    let commit_short = if commit.len() >= 7 {
        commit[..7].to_string()
    } else {
        commit_short
    };

    println!("cargo:rustc-env=GIT_COMMIT={commit}");
    println!("cargo:rustc-env=GIT_COMMIT_SHORT={commit_short}");
    println!("cargo:rustc-env=GIT_TAG={tag}");
    println!("cargo:rustc-env=GIT_DESCRIBE={describe}");
    println!(
        "cargo:rustc-env=APP_VERSION={}",
        env!("CARGO_PKG_VERSION")
    );
}

fn git_out(args: impl IntoIterator<Item = &'static str>) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn embed_windows_icon() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/app_icon.ico");
    res.set("ProductName", "Sergas ZIP Shrinker");
    res.set("FileDescription", "Sergas ZIP Shrinker");
    if let Err(err) = res.compile() {
        println!("cargo:warning=Windows icon resource not embedded: {err}");
    }
}
