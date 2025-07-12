use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serenity::model::id::GuildId;
use std::time::Duration;
use tokio::process::Command;
use tracing::error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub title: String,
    pub url: String,
    pub duration: Option<u64>,
    pub thumbnail: Option<String>,
    pub uploader: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CurrentTrack {
    pub info: TrackInfo,
    pub start_time: DateTime<Utc>,
    pub paused_at: Option<DateTime<Utc>>,
    pub total_pause_duration: Duration,
    pub is_playing: bool,
}

impl CurrentTrack {
    pub fn new(info: TrackInfo) -> Self {
        Self {
            info,
            start_time: Utc::now(),
            paused_at: None,
            total_pause_duration: Duration::ZERO,
            is_playing: true,
        }
    }

    pub fn pause(&mut self) {
        if self.is_playing {
            self.paused_at = Some(Utc::now());
            self.is_playing = false;
        }
    }

    pub fn resume(&mut self) {
        if !self.is_playing {
            if let Some(paused_at) = self.paused_at.take() {
                let pause_duration = Utc::now().signed_duration_since(paused_at);
                self.total_pause_duration += pause_duration.to_std().unwrap_or(Duration::ZERO);
            }
            self.is_playing = true;
        }
    }

    pub fn get_current_position(&self) -> Duration {
        let total_elapsed = Utc::now().signed_duration_since(self.start_time);
        let mut position = total_elapsed.to_std().unwrap_or(Duration::ZERO);
        
        // Subtract total pause time
        position = position.saturating_sub(self.total_pause_duration);
        
        // If currently paused, don't count time since pause
        if let Some(paused_at) = self.paused_at {
            let current_pause = Utc::now().signed_duration_since(paused_at);
            position = position.saturating_sub(current_pause.to_std().unwrap_or(Duration::ZERO));
        }
        
        position
    }
}

pub struct MusicManager {
    queues: DashMap<GuildId, Vec<TrackInfo>>,
    current_tracks: DashMap<GuildId, CurrentTrack>,
}

impl MusicManager {
    pub fn new() -> Self {
        Self {
            queues: DashMap::new(),
            current_tracks: DashMap::new(),
        }
    }

    pub async fn add_to_queue(&self, guild_id: GuildId, track: TrackInfo) {
        self.queues.entry(guild_id).or_insert_with(Vec::new).push(track);
    }

    pub async fn get_queue(&self, guild_id: GuildId) -> Vec<TrackInfo> {
        self.queues.get(&guild_id).map(|q| q.clone()).unwrap_or_default()
    }

    pub async fn clear_queue(&self, guild_id: GuildId) {
        self.queues.remove(&guild_id);
        self.current_tracks.remove(&guild_id);
    }

    pub async fn skip_current(&self, guild_id: GuildId) -> Option<TrackInfo> {
        let mut queue = self.queues.entry(guild_id).or_insert_with(Vec::new);
        if !queue.is_empty() {
            Some(queue.remove(0))
        } else {
            None
        }
    }

    pub async fn shuffle_queue(&self, guild_id: GuildId) {
        use rand::seq::SliceRandom;
        use rand::thread_rng;
        
        if let Some(mut queue) = self.queues.get_mut(&guild_id) {
            queue.shuffle(&mut thread_rng());
        }
    }

    pub async fn set_current_track(&self, guild_id: GuildId, track: TrackInfo) {
        self.current_tracks.insert(guild_id, CurrentTrack::new(track));
    }

    pub async fn get_current_track(&self, guild_id: GuildId) -> Option<CurrentTrack> {
        self.current_tracks.get(&guild_id).map(|t| t.clone())
    }

    pub async fn pause_current(&self, guild_id: GuildId) {
        if let Some(mut track) = self.current_tracks.get_mut(&guild_id) {
            track.pause();
        }
    }

    pub async fn resume_current(&self, guild_id: GuildId) {
        if let Some(mut track) = self.current_tracks.get_mut(&guild_id) {
            track.resume();
        }
    }

    pub async fn queue_length(&self, guild_id: GuildId) -> usize {
        self.queues.get(&guild_id).map(|q| q.len()).unwrap_or(0)
    }
}

pub async fn extract_track_info(query: &str) -> Result<TrackInfo> {
    let output = if crate::utils::is_youtube_url(query) {
        // Direct URL
        Command::new("yt-dlp")
            .args(&[
                "--dump-json",
                "--no-playlist",
                query
            ])
            .output()
            .await?
    } else {
        // Search query
        Command::new("yt-dlp")
            .args(&[
                "--dump-json",
                "--no-playlist",
                &format!("ytsearch1:{}", query)
            ])
            .output()
            .await?
    };

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        error!("yt-dlp error: {}", error);
        return Err(anyhow::anyhow!("Failed to extract track info: {}", error));
    }

    let json_output = String::from_utf8(output.stdout)?;
    let video_info: serde_json::Value = serde_json::from_str(&json_output)?;

    Ok(TrackInfo {
        title: video_info["title"].as_str().unwrap_or("Unknown").to_string(),
        url: video_info["webpage_url"].as_str().unwrap_or(
            video_info["original_url"].as_str().unwrap_or(query)
        ).to_string(),
        duration: video_info["duration"].as_u64(),
        thumbnail: video_info["thumbnail"].as_str().map(|s| s.to_string()),
        uploader: video_info["uploader"].as_str().map(|s| s.to_string()),
    })
}

pub async fn search_youtube(query: &str, limit: usize) -> Result<Vec<TrackInfo>> {
    let output = Command::new("yt-dlp")
        .args(&[
            "--dump-json",
            "--no-playlist",
            &format!("ytsearch{}:{}", limit, query)
        ])
        .output()
        .await?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Search failed: {}", error));
    }

    let json_output = String::from_utf8(output.stdout)?;
    let mut tracks = Vec::new();

    // yt-dlp outputs one JSON object per line for searches
    for line in json_output.lines() {
        if let Ok(video_info) = serde_json::from_str::<serde_json::Value>(line) {
            tracks.push(TrackInfo {
                title: video_info["title"].as_str().unwrap_or("Unknown").to_string(),
                url: video_info["webpage_url"].as_str().unwrap_or("").to_string(),
                duration: video_info["duration"].as_u64(),
                thumbnail: video_info["thumbnail"].as_str().map(|s| s.to_string()),
                uploader: video_info["uploader"].as_str().map(|s| s.to_string()),
            });
        }
    }

    Ok(tracks)
}

pub async fn get_stream_url(url: &str) -> Result<String> {
    let output = Command::new("yt-dlp")
        .args(&[
            "--get-url",
            "--format", "bestaudio[ext=m4a]/bestaudio[ext=mp3]/bestaudio",
            url
        ])
        .output()
        .await?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to get stream URL: {}", error));
    }

    let stream_url = String::from_utf8(output.stdout)?.trim().to_string();
    Ok(stream_url)
}

pub async fn create_audio_source_from_file(url: &str) -> Result<songbird::input::Input> {
    // Download the audio to a temporary file first
    let temp_file = format!("/tmp/goonbot_audio_{}.m4a", Uuid::new_v4());
    
    let output = Command::new("yt-dlp")
        .args(&[
            "--extract-audio",
            "--audio-format", "m4a",
            "--audio-quality", "0",
            "--output", &temp_file,
            url
        ])
        .output()
        .await?;
    
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to download audio: {}", error));
    }
    
    // Create input from the downloaded file
    let source = songbird::input::File::new(temp_file.clone());
    let input = songbird::input::Input::from(source);
    
    // Schedule cleanup of temp file (after a delay to ensure playback started)
    let cleanup_file = temp_file.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(300)).await; // 5 minutes
        let _ = tokio::fs::remove_file(cleanup_file).await;
    });
    
    Ok(input)
}

pub async fn create_audio_source(url: &str) -> Result<songbird::input::Input> {
    // Try the file-based approach first
    create_audio_source_from_file(url).await
}
