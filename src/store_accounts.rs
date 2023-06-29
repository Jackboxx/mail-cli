use std::{collections::HashMap, fs};

use serde::{Deserialize, Serialize};

use crate::utils::get_data_dir_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccounts(HashMap<String, StoredAccountData>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccountData {
    pub access_token: String,
    pub refresh_token: String,
}

impl StoredAccounts {
    pub fn map(&self) -> &HashMap<String, StoredAccountData> {
        &self.0
    }
    pub fn insert(&mut self, k: String, v: StoredAccountData) -> anyhow::Result<()> {
        self.0.insert(k, v);
        self.store_data()
    }

    pub fn store_data(&self) -> anyhow::Result<()> {
        let path = get_data_dir_path()?;

        fs::create_dir_all(&path)?;
        fs::write(path.join("accounts.toml"), toml::to_string_pretty(self)?)?;

        Ok(())
    }
}

impl StoredAccountData {
    pub fn new(access_token: String, refresh_token: String) -> Self {
        Self {
            access_token,
            refresh_token,
        }
    }
}
