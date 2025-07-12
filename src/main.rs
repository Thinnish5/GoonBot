use anyhow::Result;
use dashmap::DashMap;
use lazy_static::lazy_static;
use poise::serenity_prelude as serenity;
use serenity::{prelude::*, model::prelude::*};
use songbird::SerenityInit;
use std::{
    collections::HashMap,
    sync::Arc,
};
use tokio::fs;
use tracing::{error, info};

mod music;
mod utils;

use music::MusicManager;
use utils::save_guild_prefix;

// Global state
lazy_static! {
    static ref MUSIC_MANAGER: Arc<MusicManager> = Arc::new(MusicManager::new());
    static ref GUILD_PREFIXES: DashMap<GuildId, String> = DashMap::new();
}

const DEFAULT_PREFIX: &str = "!";
const BUILD_DATE: &str = "2025-01-07";

struct Data {
    music_manager: Arc<MusicManager>,
}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

/// Main goon command group
#[poise::command(prefix_command, subcommands("search", "queue_cmd", "skip", "leave", "help_cmd", "stop"))]
async fn goon(
    ctx: Context<'_>,
    #[description = "YouTube URL or search query"] 
    #[rest] query: Option<String>,
) -> Result<(), Error> {
    if let Some(query) = query {
        // If query provided, treat as play command
        play_song(ctx, query).await
    } else {
        ctx.say("Use `!goon help` to see available commands, or `!goon <query>` to play a song").await?;
        Ok(())
    }
}

/// Search and play a song from YouTube (subcommand of goon)
#[poise::command(prefix_command, rename = "search")]
async fn search(
    ctx: Context<'_>,
    #[description = "YouTube URL or search query"] 
    #[rest] query: String,
) -> Result<(), Error> {
    play_song(ctx, query).await
}

async fn play_song(ctx: Context<'_>, query: String) -> Result<(), Error> {
    info!("Play command called with query: {}", query);
    
    let guild_id = ctx.guild_id().ok_or("This command can only be used in a server, not in DMs")?;
    info!("Command used in guild: {}", guild_id);
    
    // Check if user is in voice channel
    let channel_id = {
        let guild = ctx.guild().ok_or("Could not access server information. Make sure the bot has proper permissions.")?;
        info!("Successfully accessed guild information");
        
        guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id)
            .ok_or("You must be in a voice channel to use this command")?
    };
    info!("User is in voice channel: {}", channel_id);

    // Join voice channel
    let manager = songbird::get(ctx.serenity_context()).await.unwrap();
    let handler = manager.join(guild_id, channel_id).await?;
    info!("Successfully joined voice channel");

    // Send processing message
    let reply = ctx.say(format!("🔍 Processing: `{}`...", query)).await?;

    // Extract track info
    info!("Attempting to extract track info for: {}", query);
    match music::extract_track_info(&query).await {
        Ok(track_info) => {
            info!("Successfully extracted track info: {}", track_info.title);
            // Add to queue
            ctx.data().music_manager.add_to_queue(guild_id, track_info.clone()).await;

            // Check if already playing
            let is_playing = {
                let handler = handler.lock().await;
                handler.queue().current().is_some()
            };
            info!("Is already playing: {}", is_playing);

            if !is_playing {
                // Start playing - try file-based approach for reliability
                info!("Starting playback for: {} (URL: {})", track_info.title, track_info.url);
                
                // Try our custom file-based approach first
                info!("Trying file-based approach (download then play)");
                match music::create_audio_source(&track_info.url).await {
                    Ok(source) => {
                        let mut handler = handler.lock().await;
                        let track_handle = handler.enqueue(source.into()).await;
                        info!("Successfully enqueued track with file-based approach");
                        
                        let uuid = track_handle.uuid();
                        info!("Track UUID: {:?}", uuid);
                        
                        // Check track state after a moment
                        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                        let track_state = track_handle.get_info().await;
                        info!("Track state after enqueue: {:?}", track_state.as_ref().map(|t| &t.playing));
                        
                        ctx.data().music_manager.set_current_track(guild_id, track_info.clone()).await;
                        
                        reply.edit(ctx, poise::CreateReply::default().content(format!("✅ Now playing: `{}`", track_info.title))).await?;
                    }
                    Err(e) => {
                        error!("File-based approach failed: {}", e);
                        // Fallback to YoutubeDl approach
                        info!("Falling back to YoutubeDl approach");
                        let source = songbird::input::YoutubeDl::new(reqwest::Client::new(), track_info.url.clone());
                        
                        let mut handler = handler.lock().await;
                        let track_handle = handler.enqueue(source.into()).await;
                        info!("Successfully enqueued track with YoutubeDl fallback");
                        
                        let uuid = track_handle.uuid();
                        info!("YoutubeDl Track UUID: {:?}", uuid);
                        
                        ctx.data().music_manager.set_current_track(guild_id, track_info.clone()).await;
                        
                        reply.edit(ctx, poise::CreateReply::default().content(format!("✅ Now playing: `{}` (fallback)", track_info.title))).await?;
                    }
                }
            } else {
                reply.edit(ctx, poise::CreateReply::default().content(format!("✅ Added to queue: `{}`", track_info.title))).await?;
            }
        }
        Err(e) => {
            error!("Failed to extract track info: {}", e);
            reply.edit(ctx, poise::CreateReply::default().content(format!("❌ Error: {}", e))).await?;
        }
    }

    Ok(())
}

/// Show the current queue (subcommand of goon)
#[poise::command(prefix_command, rename = "queue")]
async fn queue_cmd(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("This command can only be used in a server, not in DMs")?;
    let queue = ctx.data().music_manager.get_queue(guild_id).await;
    
    if queue.is_empty() {
        ctx.say("The queue is empty.").await?;
        return Ok(());
    }

    let mut description = String::new();
    for (i, track) in queue.iter().take(10).enumerate() {
        description.push_str(&format!("{}. {}\n", i + 1, track.title));
    }

    if queue.len() > 10 {
        description.push_str(&format!("\n...and {} more songs", queue.len() - 10));
    }

    ctx.send(poise::CreateReply::default().embed(
        serenity::CreateEmbed::new()
            .title("Current Queue")
            .description(description)
            .color(serenity::Color::GOLD)
            .footer(serenity::CreateEmbedFooter::new(format!("{} songs total", queue.len())))
    )).await?;

    Ok(())
}

/// Skip the current song (subcommand of goon)
#[poise::command(prefix_command, rename = "skip")]
async fn skip(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("This command can only be used in a server, not in DMs")?;
    
    let manager = songbird::get(ctx.serenity_context()).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if !queue.is_empty() {
            queue.skip()?;
            ctx.say("⏭️ Skipped current song").await?;
        } else {
            ctx.say("Nothing to skip").await?;
        }
    } else {
        ctx.say("Not connected to voice channel").await?;
    }

    Ok(())
}

/// Leave the voice channel (subcommand of goon)
#[poise::command(prefix_command, rename = "leave")]
async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("This command can only be used in a server, not in DMs")?;
    
    let manager = songbird::get(ctx.serenity_context()).await.unwrap();
    if manager.get(guild_id).is_some() {
        manager.remove(guild_id).await?;
        ctx.data().music_manager.clear_queue(guild_id).await;
        ctx.say("👋 Left voice channel and cleared queue").await?;
    } else {
        ctx.say("Not connected to voice channel").await?;
    }

    Ok(())
}

/// Stop the current song and clear queue (subcommand of goon)
#[poise::command(prefix_command, rename = "stop")]
async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("This command can only be used in a server, not in DMs")?;
    
    let manager = songbird::get(ctx.serenity_context()).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if !queue.is_empty() {
            queue.stop();
            ctx.data().music_manager.clear_queue(guild_id).await;
            ctx.say("⏹️ Stopped playback and cleared queue").await?;
        } else {
            ctx.say("Nothing is playing").await?;
        }
    } else {
        ctx.say("Not connected to voice channel").await?;
    }

    Ok(())
}

/// Show help information (subcommand of goon)
#[poise::command(prefix_command, rename = "help")]
async fn help_cmd(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(poise::CreateReply::default().embed(
        serenity::CreateEmbed::new()
            .title("🎵 GoonBot Commands 🤤")
            .description("A Rust-powered music bot")
            .field("!goon <query>", "Play a song from YouTube (URL or search)", false)
            .field("!goon queue", "Show the current queue", false)
            .field("!goon skip", "Skip the current song", false)
            .field("!goon stop", "Stop playback and clear queue", false)
            .field("!goon leave", "Leave the voice channel", false)
            .field("!goon help", "Show this help message", false)
            .color(serenity::Color::BLURPLE)
            .footer(serenity::CreateEmbedFooter::new(format!("Build date: {}", BUILD_DATE)))
    )).await?;

    Ok(())
}

struct Handler;

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: serenity::Context, ready: Ready) {
        info!("Bot is ready! Logged in as {}", ready.user.name);
    }

    async fn guild_create(&self, _ctx: serenity::Context, guild: Guild, _is_new: Option<bool>) {
        // Set default prefix for new guilds
        GUILD_PREFIXES.insert(guild.id, DEFAULT_PREFIX.to_string());
        if let Err(e) = save_guild_prefix(guild.id, DEFAULT_PREFIX).await {
            error!("Failed to save prefix for guild {}: {}", guild.id, e);
        }
    }

    async fn voice_state_update(
        &self,
        ctx: serenity::Context,
        old: Option<VoiceState>,
        new: VoiceState,
    ) {
        // Handle when users leave voice channels (auto-disconnect logic)
        if let Some(old_state) = old {
            if let Some(old_channel) = old_state.channel_id {
                if new.channel_id != Some(old_channel) {
                    // User left or moved from this channel
                    if let Some(guild_id) = old_state.guild_id {
                        // Get basic info we need before the async operation
                        let member_count = {
                            if let Some(guild) = ctx.cache.guild(guild_id) {
                                if let Some(channel) = guild.channels.get(&old_channel) {
                                    channel
                                        .members(&ctx.cache)
                                        .unwrap_or_default()
                                        .iter()
                                        .filter(|m| !m.user.bot)
                                        .count()
                                } else {
                                    1 // If we can't get the channel, assume there are humans
                                }
                            } else {
                                1 // If we can't get the guild, assume there are humans
                            }
                        };

                        if member_count == 0 {
                            // Disconnect the bot if no humans are left
                            if let Some(manager) = songbird::get(&ctx).await {
                                if manager.get(guild_id).is_some() {
                                    let _ = manager.remove(guild_id).await;
                                    MUSIC_MANAGER.clear_queue(guild_id).await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Read bot token
    let token = fs::read_to_string("secret.secret")
        .await
        .expect("Failed to read bot token from secret.secret");
    let token = token.trim();

    // Create data directory if it doesn't exist
    fs::create_dir_all("data").await?;

    // Load guild prefixes
    if let Ok(prefixes_data) = fs::read_to_string("data/prefixes.json").await {
        if let Ok(prefixes) = serde_json::from_str::<HashMap<u64, String>>(&prefixes_data) {
            for (guild_id, prefix) in prefixes {
                GUILD_PREFIXES.insert(GuildId::new(guild_id), prefix);
            }
        }
    }

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILDS;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![goon()],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some(DEFAULT_PREFIX.into()),
                ..Default::default()
            },
            on_error: |error| {
                Box::pin(async move {
                    match error {
                        poise::FrameworkError::Command { error, ctx, .. } => {
                            error!("Error in command `{}`: {}", ctx.command().name, error);
                            let _ = ctx.say(format!("❌ Command error: {}", error)).await;
                        }
                        error => {
                            error!("Other error: {}", error);
                        }
                    }
                })
            },
            ..Default::default()
        })
        .setup(|_ctx, _ready, _framework| {
            Box::pin(async move {
                info!("Bot setup complete - prefix commands only");
                Ok(Data {
                    music_manager: MUSIC_MANAGER.clone(),
                })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Error creating client");

    info!("Starting GoonBot...");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }

    Ok(())
}
