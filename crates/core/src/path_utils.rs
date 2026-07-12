use std::path::{Path, PathBuf};

pub fn normalize_path_for_storage(path: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        if let Ok(relative) = path.strip_prefix(&home) {
            return if relative.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", relative.display())
            };
        }
    }
    path.display().to_string()
}

pub fn is_private_name(name: &str) -> bool {
    matches!(
        name,
        "Documents" | "Desktop" | "Downloads" | "Pictures" | "Movies" | "Music"
    )
}

pub fn safe_sample_names(samples: &[String]) -> Vec<String> {
    samples
        .iter()
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            let extension = Path::new(&lower)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            matches!(
                extension,
                "json"
                    | "yaml"
                    | "yml"
                    | "toml"
                    | "xml"
                    | "plist"
                    | "db"
                    | "sqlite"
                    | "lock"
                    | "ini"
                    | "conf"
                    | "config"
            ) || matches!(lower.as_str(), "config" | "settings" | "preferences")
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::safe_sample_names;

    #[test]
    fn sample_filter_is_allow_list_based() {
        let samples = vec![
            "resume.pdf".into(),
            "config.json".into(),
            "photo.jpg".into(),
            "state.db".into(),
        ];
        assert_eq!(safe_sample_names(&samples), vec!["config.json", "state.db"]);
    }
}
