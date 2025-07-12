use anyhow::Result;
use serde_json;
use serenity::model::id::GuildId;
use std::collections::HashMap;
use tokio::fs;

const PREFIXES_FILE: &str = "data/prefixes.json";

pub async fn get_guild_prefix(guild_id: GuildId) -> Result<String> {
    let prefixes_data = fs::read_to_string(PREFIXES_FILE).await?;
    let prefixes: HashMap<u64, String> = serde_json::from_str(&prefixes_data)?;
    Ok(prefixes.get(&guild_id.get()).cloned().unwrap_or_else(|| "!".to_string()))
}

pub async fn save_guild_prefix(guild_id: GuildId, prefix: &str) -> Result<()> {
    let prefixes_data = fs::read_to_string(PREFIXES_FILE).await.unwrap_or_else(|_| "{}".to_string());
    let mut prefixes: HashMap<u64, String> = serde_json::from_str(&prefixes_data).unwrap_or_default();
    
    prefixes.insert(guild_id.get(), prefix.to_string());
    
    let updated_data = serde_json::to_string_pretty(&prefixes)?;
    fs::write(PREFIXES_FILE, updated_data).await?;
    
    Ok(())
}

pub fn format_duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{}:{:02}", minutes, seconds)
}

pub fn create_progress_bar(current: u64, total: u64, length: usize) -> String {
    if total == 0 {
        return "┈".repeat(length);
    }
    
    let current = current.min(total);
    let percentage = current as f64 / total as f64;
    let position = (percentage * length as f64) as usize;
    
    let filled = "━".repeat(position);
    let empty = "┈".repeat(length.saturating_sub(position + 1));
    
    format!("{}⚪{}", filled, empty)
}

pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com") || url.contains("youtu.be")
}

pub fn extract_playlist_id(url: &str) -> Option<String> {
    use regex::Regex;
    let re = Regex::new(r"list=([^&]+)").unwrap();
    re.captures(url).and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
}
