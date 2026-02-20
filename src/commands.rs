use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum StartMode {
    Normal,
    Sudo,
}

impl Default for StartMode {
    fn default() -> Self {
        StartMode::Normal
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MagicWord {
    /// The keyword that triggers this magic word.
    pub keyword: String,
    /// The command, URL, or "@copy@".
    pub filename_or_url: String,
    /// Normal or Sudo.
    #[serde(default)]
    pub start_mode: StartMode,
    /// Working directory for the command.
    pub start_path: Option<String>,
    /// Default parameters. Use $W$ as a placeholder for user-supplied arguments.
    pub parameters: Option<String>,
}

impl MagicWord {
    /// Check if the filename_or_url looks like a URL.
    fn is_url(s: &str) -> bool {
        s.starts_with("http://")
            || s.starts_with("https://")
            || s.starts_with("ftp://")
            || s.starts_with("www.")
    }

    /// Check if this is the @copy@ special command.
    fn is_copy_command(&self) -> bool {
        self.filename_or_url.eq_ignore_ascii_case("@copy@")
    }

    /// Resolve the final parameters by substituting $W$ with user args.
    /// If there's no $W$ in the default parameters, user_args are appended.
    fn resolve_params(&self, user_args: &str) -> String {
        if let Some(default_params) = &self.parameters {
            if default_params.contains("$W$") {
                default_params.replace("$W$", user_args)
            } else if user_args.is_empty() {
                default_params.clone()
            } else {
                format!("{} {}", default_params, user_args)
            }
        } else {
            user_args.to_string()
        }
    }

    /// Resolve filename_or_url by substituting $W$ with user args.
    fn resolve_filename(&self, user_args: &str) -> String {
        if self.filename_or_url.contains("$W$") {
            self.filename_or_url.replace("$W$", user_args)
        } else {
            self.filename_or_url.clone()
        }
    }

    /// Check if this magic word uses $W$ substitution.
    pub fn needs_w_input(&self) -> bool {
        self.filename_or_url.contains("$W$")
            || self
                .parameters
                .as_ref()
                .is_some_and(|p| p.contains("$W$"))
    }

    /// Execute this magic word with the given user arguments (text after the keyword).
    pub fn execute(&self, user_args: &str) {
        // @copy@ — copy resolved params to clipboard
        if self.is_copy_command() {
            let text = self.resolve_params(user_args);
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(&text);
            }
            return;
        }

        let filename = self.resolve_filename(user_args);
        let params = self.resolve_params(user_args);

        // URL — open in default browser
        if Self::is_url(&filename) {
            let url = if params.is_empty() {
                filename
            } else if filename.contains("$W$") {
                // $W$ was already resolved above
                filename
            } else {
                // Append params as query or just pass them
                format!("{}{}", filename, params)
            };
            let _ = std::process::Command::new("xdg-open")
                .arg(&url)
                .spawn();
            return;
        }

        // Directory — open in file manager
        if std::path::Path::new(&filename).is_dir() {
            let _ = std::process::Command::new("xdg-open")
                .arg(&filename)
                .spawn();
            return;
        }

        // Regular command
        let full_command = if params.is_empty() {
            filename.clone()
        } else {
            format!("{} {}", filename, params)
        };

        let shell_cmd = match self.start_mode {
            StartMode::Sudo => format!("sudo {}", full_command),
            StartMode::Normal => full_command,
        };

        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(&shell_cmd);
        if let Some(path) = &self.start_path {
            if !path.is_empty() {
                cmd.current_dir(path);
            }
        }
        let _ = cmd.spawn();
    }

}

/// Find a magic word by exact keyword match (case-insensitive).
pub fn find_by_keyword<'a>(magic_words: &'a [MagicWord], keyword: &str) -> Option<&'a MagicWord> {
    magic_words
        .iter()
        .find(|mw| mw.keyword.eq_ignore_ascii_case(keyword))
}
