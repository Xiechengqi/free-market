pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_SHA: &str = env!("FREEMARKET_BUILD_SHA");
pub const BUILD_TIME: &str = env!("FREEMARKET_BUILD_TIME");
pub const COMMIT_MESSAGE: &str = env!("FREEMARKET_COMMIT_MESSAGE");

pub fn target_triple() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub fn version_text() -> String {
    format!(
        "free-market {}\n\
         package: {}\n\
         commit: {}\n\
         commit message: {}\n\
         built: {}\n\
         target: {}",
        PACKAGE_VERSION,
        PACKAGE_VERSION,
        BUILD_SHA,
        COMMIT_MESSAGE,
        BUILD_TIME,
        target_triple(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_text_includes_package_version() {
        assert!(version_text().contains(PACKAGE_VERSION));
    }
}
