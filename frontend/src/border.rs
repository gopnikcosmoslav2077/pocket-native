//! the border behind the screens: none, the stock one, or a PNG from the
//! borders folder, as the Display menu's Border names it.

use std::path::PathBuf;

/// the setting for the stock border; an empty one is none, any other a
/// file's name in the borders folder.
pub const STOCK: &str = "stock";

/// POCKET_NATIVE_BORDERS, beside the games on a handheld's card and in the
/// app's folder on Android, or borders in the configuration directory.
pub fn dir() -> PathBuf {
    std::env::var_os("POCKET_NATIVE_BORDERS").map(PathBuf::from).unwrap_or_else(|| crate::settings::config_dir().join("borders"))
}

/// what Border steps through: none, the stock one, then each PNG in the
/// borders folder by name.
pub fn choices() -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.') && name.to_lowercase().ends_with(".png"))
        .collect();
    files.sort_by_key(|name| name.to_lowercase());
    [String::new(), STOCK.to_owned()].into_iter().chain(files).collect()
}

/// how the menu shows a setting: Off, Stock, or the file's name without
/// .png.
pub fn label(setting: &str) -> String {
    match setting {
        "" => "Off".to_owned(),
        STOCK => "Stock".to_owned(),
        file => file.strip_suffix(".png").or_else(|| file.strip_suffix(".PNG")).unwrap_or(file).to_owned(),
    }
}

/// the next or last choice from setting.
pub fn step(setting: &str, by: isize) -> String {
    let choices = choices();
    let at = choices.iter().position(|choice| choice == setting).unwrap_or(0) as isize;
    choices[(at + by).rem_euclid(choices.len() as isize) as usize].clone()
}

/// has the GPU draw the border the setting names, once each time it
/// changes: a file that is gone or not a PNG leaves none, and says so.
pub fn apply(setting: &str) {
    static APPLIED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    let mut applied = APPLIED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if applied.as_deref() == Some(setting) {
        return;
    }
    *applied = Some(setting.to_owned());
    let border = match setting {
        "" => None,
        STOCK => Some(zakuro_gpu::Border::Stock),
        file => {
            let path = dir().join(file);
            match zakuro_gpu::load_border(&path) {
                Ok(border) => {
                    log::info!("border: {}", path.display());
                    Some(border)
                }
                Err(error) => {
                    log::warn!("could not use {} as a border, {error}", path.display());
                    None
                }
            }
        }
    };
    zakuro_gpu::set_border(border);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_steps_through_off_stock_and_the_folders_pictures() {
        let folder = std::env::temp_dir().join(format!("pocket-native-borders-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        for name in ["Wood.png", "arcade.PNG", "notes.txt", ".hidden.png"] {
            std::fs::write(folder.join(name), b"").unwrap();
        }
        std::env::set_var("POCKET_NATIVE_BORDERS", &folder);
        assert_eq!(choices(), ["", STOCK, "arcade.PNG", "Wood.png"]);
        assert_eq!(step("", 1), STOCK);
        assert_eq!(step("", -1), "Wood.png");
        assert_eq!(step("Wood.png", 1), "");
        // a file taken away starts the steps over
        assert_eq!(step("gone.png", 1), STOCK);
        assert_eq!([label(""), label(STOCK), label("arcade.PNG"), label("Wood.png")], ["Off", "Stock", "arcade", "Wood"]);
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
