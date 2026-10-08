//! The web view's page, written to disk by the converter and the editor.
//!
//! It is opened as a file, in OpenKneeboard's Single file tab or a browser on
//! this PC, never from the converter. The converter starts with a mission and
//! OpenKneeboard loads its tabs long before that, and a page the converter
//! served would fail to load until then and sit on Chromium's error page,
//! whose reload backs off to minutes. A file always loads: it says it is
//! waiting, and draws as soon as the converter answers at
//! [`crate::WEB_ADDRESS`].

use std::path::Path;

/// The page: the grid drawn in the aircraft's CDU font, kept up to date by a
/// long poll.
pub const PAGE: &str = include_str!("kneeboard-cdu.html");

/// The file the page is written to, beside the settings.
pub const PAGE_FILE: &str = "kneeboard-cdu.html";

/// Write the page to `path`, unless it is already there as it is. Left alone
/// when unchanged, because OpenKneeboard reloads a file tab whenever its file
/// is written.
pub fn write_page(path: &Path) -> std::io::Result<()> {
    let wanted = PAGE.replace("\r\n", "\n").replace('\n', "\r\n");
    if std::fs::read_to_string(path).is_ok_and(|held| held == wanted) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, wanted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opened from disk, the page asks the converter at this address, so it
    /// must be the one the converter serves on.
    #[test]
    fn the_page_asks_the_address_the_converter_serves() {
        let address = format!("\"http://{}\"", crate::WEB_ADDRESS);
        assert!(PAGE.contains(&address), "the page does not name {address}");
    }

    #[test]
    fn an_unchanged_page_is_not_written_again() {
        let dir = std::env::temp_dir().join(format!("dsc-web-{}", std::process::id()));
        let path = dir.join(PAGE_FILE);
        write_page(&path).unwrap();
        let first = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_page(&path).unwrap();
        let second = std::fs::metadata(&path).unwrap().modified().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(first, second);
    }
}
