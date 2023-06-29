use std::path::PathBuf;

use anyhow::anyhow;

/// writes user data to `user.toml` file creating all parent directories in the process
pub fn get_data_dir_path() -> anyhow::Result<PathBuf> {
    if let Some(base_dir) = directories::BaseDirs::new() {
        Ok(base_dir.data_dir().join("mail-cli/"))
    } else {
        Err(anyhow!("failed to find home directory"))
    }
}
