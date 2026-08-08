//! Build identity and project links shown in the About overlay.

pub const APP_NAME: &str = "Sergas ZIP Shrinker";
pub const VERSION: &str = env!("APP_VERSION");
pub const GIT_TAG: &str = env!("GIT_TAG");
pub const GIT_COMMIT: &str = env!("GIT_COMMIT");
pub const GIT_COMMIT_SHORT: &str = env!("GIT_COMMIT_SHORT");
pub const GIT_DESCRIBE: &str = env!("GIT_DESCRIBE");

/// Public GitHub repository for this project.
pub const GITHUB_URL: &str = "https://github.com/pvianag/encolledor_imaxes_sergas";

pub fn release_url() -> String {
    if GIT_TAG.starts_with('v') {
        format!("{GITHUB_URL}/releases/tag/{GIT_TAG}")
    } else {
        format!("{GITHUB_URL}/releases")
    }
}

pub fn tag_url() -> String {
    format!("{GITHUB_URL}/releases/tag/{GIT_TAG}")
}

pub fn commit_url() -> String {
    format!("{GITHUB_URL}/commit/{GIT_COMMIT}")
}

pub fn open_url(url: &str) {
    let _ = webbrowser::open(url);
}
