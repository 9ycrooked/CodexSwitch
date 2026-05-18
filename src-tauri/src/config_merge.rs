use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use toml_edit::{DocumentMut, Item, Table};

use crate::error::{stringify_io, AppResult};

pub fn merge_config_files(current_path: &Path, account_path: &Path) -> AppResult<String> {
    let current_text = if current_path.exists() {
        fs::read_to_string(current_path).map_err(stringify_io)?
    } else {
        String::new()
    };
    let account_text = if account_path.exists() {
        fs::read_to_string(account_path).map_err(stringify_io)?
    } else {
        String::new()
    };
    merge_config_text(&current_text, &account_text)
}

pub fn merge_config_text(current_text: &str, account_text: &str) -> AppResult<String> {
    let mut current = if current_text.trim().is_empty() {
        DocumentMut::new()
    } else {
        parse_toml(current_text)?
    };
    let account = if account_text.trim().is_empty() {
        DocumentMut::new()
    } else {
        parse_toml(account_text)?
    };

    merge_item(current.as_table_mut(), account.as_table());
    Ok(current.to_string())
}

fn merge_item(current: &mut Table, account: &Table) {
    let keys: BTreeSet<String> = account.iter().map(|(key, _)| key.to_string()).collect();
    for key in keys {
        let account_item = &account[&key];
        if !current.contains_key(&key) {
            current.insert(&key, account_item.clone());
            continue;
        }
        if let (Some(current_table), Some(account_table)) =
            (current[&key].as_table_mut(), account_item.as_table())
        {
            merge_item(current_table, account_table);
        } else if let (Item::ArrayOfTables(current_tables), Item::ArrayOfTables(account_tables)) =
            (&mut current[&key], account_item)
        {
            if current_tables.is_empty() {
                *current_tables = account_tables.clone();
            }
        }
    }
}

pub(crate) fn parse_toml(text: &str) -> AppResult<DocumentMut> {
    text.parse::<DocumentMut>()
        .map_err(|err| format!("TOML 解析失败：{err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_toml_current_first() {
        let current = r#"
model = "gpt-5.5"

[plugins."browser@openai-bundled"]
enabled = true

[mcp_servers.local]
command = "current"
"#;
        let account = r#"
model = "gpt-4.1"

[plugins."superpowers@openai-curated"]
enabled = true

[plugins."browser@openai-bundled"]
enabled = false

[mcp_servers.local]
command = "account"

[mcp_servers.extra]
command = "extra"
"#;

        let merged = merge_config_text(current, account).unwrap();
        let parsed = parse_toml(&merged).unwrap();
        assert_eq!(parsed["model"].as_str(), Some("gpt-5.5"));
        assert_eq!(
            parsed["plugins"]["browser@openai-bundled"]["enabled"].as_bool(),
            Some(true)
        );
        assert_eq!(
            parsed["plugins"]["superpowers@openai-curated"]["enabled"].as_bool(),
            Some(true)
        );
        assert_eq!(
            parsed["mcp_servers"]["local"]["command"].as_str(),
            Some("current")
        );
        assert_eq!(
            parsed["mcp_servers"]["extra"]["command"].as_str(),
            Some("extra")
        );
    }
}
