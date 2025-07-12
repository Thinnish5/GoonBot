use crate::{music::*, utils::*, MUSIC_MANAGER, GUILD_PREFIXES, BUILD_DATE};
use anyhow::Result;
use rand::seq::SliceRandom;
use serenity::{
    builder::{CreateEmbed, CreateEmbedFooter, CreateMessage},
    framework::standard::{macros::command, Args, CommandResult},
    model::{channel::Message, Colour},
    prelude::*,
};
use songbird::{input::Restartable, Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{error, info};

struct TrackEndNotifier {
    guild_id: serenity::model::id::GuildId,
    ctx: Arc<Context>,
    channel_id: serenity::model::id::ChannelId,
}

#[serenity::async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(_track_list) = ctx {
            // Play next song in queue
            let _ = play_next_in_queue(self.ctx.clone(), self.guild_id, self.channel_id).await;
        }
        None
    }
}

async fn play_next_in_queue(
    ctx: Arc<Context>,
    guild_id: serenity::model::id::GuildId,
    channel_id: serenity::model::id::ChannelId,
) -> Result<()> {
    let next_track = MUSIC_MANAGER.skip_current(guild_id).await;
    
    if let Some(track_info) = next_track {
        let stream_url = get_stream_url(&track_info.url).await?;
        
        let manager = songbird::get(&ctx).await.unwrap();
        if let Some(handler_lock) = manager.get(guild_id) {
            let mut handler = handler_lock.lock().await;
            
            let source = songbird::input::Input::new(
                true, // is_live
                songbird::input::Reader::Extension(Box::new(
                    songbird::input::codecs::OpusDecoder::new(
                        songbird::input::codecs::DcaReader::new(
                            tokio_util::io::StreamReader::new(
                                reqwest::get(&stream_url)
                                    .await?
                                    .bytes_stream()
                                    .map(|result| {
                                        result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                                    })
                            )
                        )?
                    )?
                )),
                songbird::input::Codec::OpusOgg,
                songbird::input::Container::Ogg,
                None,
            );
            
            let track_handle = handler.play_source(source.into());
            
            // Add event handler for when this track ends
            let _ = track_handle.add_event(
                Event::Track(TrackEvent::End),
                TrackEndNotifier {
                    guild_id,
                    ctx: ctx.clone(),
                    channel_id,
                },
            );
            
            MUSIC_MANAGER.set_current_track(guild_id, track_info.clone()).await;
            
            // Send now playing message
            send_now_playing_message(&ctx, channel_id, &track_info).await?;
        }
    }
    
    Ok(())
}

async fn send_now_playing_message(
    ctx: &Context,
    channel_id: serenity::model::id::ChannelId,
    track: &TrackInfo,
) -> Result<()> {
    let mut embed = CreateEmbed::new()
        .title("🎵 Now Gooning")
        .description(&track.title)
        .color(Colour::BLURPLE);
    
    if let Some(thumbnail) = &track.thumbnail {
        embed = embed.thumbnail(thumbnail);
    }
    
    if let Some(duration) = track.duration {
        embed = embed.field("Duration", format_duration(duration), true);
    }
    
    if let Some(uploader) = &track.uploader {
        embed = embed.field("Channel", uploader, true);
    }
    
    let message = CreateMessage::new().embed(embed);
    
    channel_id.send_message(&ctx.http, message).await?;
    
    Ok(())
}

#[command]
#[aliases("g")]
async fn goon(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.rest();
    if query.is_empty() {
        msg.channel_id.say(&ctx.http, "Please provide a search query. Usage: `!goon <query>`").await?;
        return Ok(());
    }

    let guild_id = msg.guild_id.unwrap();
    let channel_id = msg.channel_id;

    // Check if user is in voice channel
    let guild = ctx.cache.guild(guild_id).unwrap();
    let voice_state = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|vs| vs.channel_id);

    let user_channel = match voice_state {
        Some(channel) => channel,
        None => {
            msg.channel_id.say(&ctx.http, "You must be in a voice channel to use this command!").await?;
            return Ok(());
        }
    };

    // Connect to voice channel if not already connected
    let manager = songbird::get(ctx).await.unwrap();
    let (handler_lock, success) = manager.join(guild_id, user_channel).await;

    if success.is_err() {
        msg.channel_id.say(&ctx.http, "Failed to join voice channel").await?;
        return Ok(());
    }

    // Send processing message
    let processing_msg = msg.channel_id.say(&ctx.http, format!("🔍 Processing: `{}`...", query)).await?;

    // Extract track info
    let track_info = match extract_track_info(query).await {
        Ok(info) => info,
        Err(e) => {
            processing_msg.edit(&ctx.http, format!("❌ Error: {}", e)).await?;
            return Ok(());
        }
    };

    // Add to queue
    MUSIC_MANAGER.add_to_queue(guild_id, track_info.clone()).await;

    // If nothing is playing, start playing
    let is_playing = {
        let handler = handler_lock.lock().await;
        !handler.queue().is_empty()
    };

    if !is_playing {
        let stream_url = match get_stream_url(&track_info.url).await {
            Ok(url) => url,
            Err(e) => {
                processing_msg.edit(&ctx.http, format!("❌ Failed to get stream: {}", e)).await?;
                return Ok(());
            }
        };

        let mut handler = handler_lock.lock().await;
        
        // Create a simple HTTP source for now
        let source = match Restartable::http(&ctx.http, &stream_url).await {
            Ok(source) => source,
            Err(e) => {
                processing_msg.edit(&ctx.http, format!("❌ Failed to create audio source: {}", e)).await?;
                return Ok(());
            }
        };

        let track_handle = handler.play_source(source.into());
        
        // Add event handler for when track ends
        let _ = track_handle.add_event(
            Event::Track(TrackEvent::End),
            TrackEndNotifier {
                guild_id,
                ctx: Arc::new(ctx.clone()),
                channel_id,
            },
        );

        MUSIC_MANAGER.set_current_track(guild_id, track_info.clone()).await;
        
        processing_msg.edit(&ctx.http, format!("✅ Now playing: `{}`", track_info.title)).await?;
    } else {
        processing_msg.edit(&ctx.http, format!("✅ Added to queue: `{}`", track_info.title)).await?;
    }

    // Delete the message after a delay
    sleep(Duration::from_secs(5)).await;
    let _ = processing_msg.delete(&ctx.http).await;

    Ok(())
}

#[command]
async fn search(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.rest();
    if query.is_empty() {
        msg.channel_id.say(&ctx.http, "Please provide a search query. Usage: `!goon search <query>`").await?;
        return Ok(());
    }

    let processing_msg = msg.channel_id.say(&ctx.http, format!("🔍 Searching for: `{}`...", query)).await?;

    let search_results = match search_youtube(query, 5).await {
        Ok(results) => results,
        Err(e) => {
            processing_msg.edit(&ctx.http, format!("❌ Search failed: {}", e)).await?;
            return Ok(());
        }
    };

    if search_results.is_empty() {
        processing_msg.edit(&ctx.http, "❌ No results found").await?;
        return Ok(());
    }

    let mut description = String::from("**Search Results:**\n");
    for (i, track) in search_results.iter().enumerate() {
        description.push_str(&format!("{}. {}\n", i + 1, track.title));
    }
    description.push_str("\nReact with 1️⃣-5️⃣ to select a song");

    let embed = CreateEmbed::new()
        .title("YouTube Search")
        .description(description)
        .color(Colour::BLUE);

    let search_msg = msg.channel_id.send_message(&ctx.http, CreateMessage::new().embed(embed)).await?;

    // Add reactions
    let reactions = ["1️⃣", "2️⃣", "3️⃣", "4️⃣", "5️⃣"];
    for (i, &reaction) in reactions.iter().enumerate() {
        if i < search_results.len() {
            search_msg.react(&ctx.http, reaction).await?;
        }
    }

    processing_msg.delete(&ctx.http).await?;

    // Set up reaction collector
    // Note: In a real implementation, you'd want to use a reaction collector
    // For now, we'll just delete the message after 30 seconds
    sleep(Duration::from_secs(30)).await;
    let _ = search_msg.delete(&ctx.http).await;

    Ok(())
}

#[command]
async fn queue(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let queue = MUSIC_MANAGER.get_queue(guild_id).await;
    
    if queue.is_empty() {
        msg.channel_id.say(&ctx.http, "The queue is empty.").await?;
        return Ok(());
    }

    let mut description = String::new();
    for (i, track) in queue.iter().take(10).enumerate() {
        description.push_str(&format!("{}. {}\n", i + 1, track.title));
    }

    if queue.len() > 10 {
        description.push_str(&format!("\n...and {} more songs", queue.len() - 10));
    }

    let embed = CreateEmbed::new()
        .title("Current Queue")
        .description(description)
        .color(Colour::GOLD)
        .footer(CreateEmbedFooter::new(format!("{} songs total", queue.len())));

    msg.channel_id.send_message(&ctx.http, CreateMessage::new().embed(embed)).await?;

    Ok(())
}

#[command]
async fn skip(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    
    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if !queue.is_empty() {
            queue.skip()?;
            msg.channel_id.say(&ctx.http, "⏭️ Skipped current song").await?;
        } else {
            msg.channel_id.say(&ctx.http, "Nothing to skip").await?;
        }
    } else {
        msg.channel_id.say(&ctx.http, "Not connected to voice channel").await?;
    }

    Ok(())
}

#[command]
async fn pause(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    
    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if !queue.is_empty() {
            if let Some(track) = queue.current() {
                let _ = track.pause();
                MUSIC_MANAGER.pause_current(guild_id).await;
                msg.channel_id.say(&ctx.http, "⏸️ Paused").await?;
            }
        } else {
            msg.channel_id.say(&ctx.http, "Nothing to pause").await?;
        }
    } else {
        msg.channel_id.say(&ctx.http, "Not connected to voice channel").await?;
    }

    Ok(())
}

#[command]
async fn resume(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    
    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if !queue.is_empty() {
            if let Some(track) = queue.current() {
                let _ = track.play();
                MUSIC_MANAGER.resume_current(guild_id).await;
                msg.channel_id.say(&ctx.http, "▶️ Resumed").await?;
            }
        } else {
            msg.channel_id.say(&ctx.http, "Nothing to resume").await?;
        }
    } else {
        msg.channel_id.say(&ctx.http, "Not connected to voice channel").await?;
    }

    Ok(())
}

#[command]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    
    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        handler.queue().stop();
        MUSIC_MANAGER.clear_queue(guild_id).await;
        msg.channel_id.say(&ctx.http, "⏹️ Stopped and cleared queue").await?;
    } else {
        msg.channel_id.say(&ctx.http, "Not connected to voice channel").await?;
    }

    Ok(())
}

#[command]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    
    let manager = songbird::get(ctx).await.unwrap();
    if manager.get(guild_id).is_some() {
        manager.remove(guild_id).await?;
        MUSIC_MANAGER.clear_queue(guild_id).await;
        msg.channel_id.say(&ctx.http, "👋 Left voice channel and cleared queue").await?;
    } else {
        msg.channel_id.say(&ctx.http, "Not connected to voice channel").await?;
    }

    Ok(())
}

#[command]
async fn shuffle(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let queue_length = MUSIC_MANAGER.queue_length(guild_id).await;
    
    if queue_length <= 1 {
        msg.channel_id.say(&ctx.http, "Need at least 2 songs in queue to shuffle").await?;
        return Ok(());
    }

    MUSIC_MANAGER.shuffle_queue(guild_id).await;
    msg.channel_id.say(&ctx.http, "🔀 Queue shuffled!").await?;

    Ok(())
}

#[command]
async fn playlist(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.rest();
    if query.is_empty() {
        msg.channel_id.say(&ctx.http, "Please provide a playlist name or URL").await?;
        return Ok(());
    }

    // Define predefined playlists
    let mut playlists = HashMap::new();
    playlists.insert("all", "https://www.youtube.com/playlist?list=PLEMIbGkCIAqDaWDz1hwg04BwSbDXDlPCh");
    playlists.insert("good", "https://www.youtube.com/playlist?list=PLEMIbGkCIAqDObuOVPqYQCyaibK8aYsI3");
    playlists.insert("loop", "https://www.youtube.com/playlist?list=PLEMIbGkCIAqDBYM493VDLLfSlsgXeBURI");
    playlists.insert("emo", "https://www.youtube.com/playlist?list=PLEMIbGkCIAqA0koaFqXvRVsed5lAs7BLF");
    playlists.insert("dl", "https://www.youtube.com/playlist?list=PLEMIbGkCIAqDDeWgpUEK7gV5wtRwgckuI");
    playlists.insert("diogo", "https://www.youtube.com/playlist?list=PL0SNMtspDuGR8cS7KNdAUYK196TAom2iJ");
    playlists.insert("undead", "https://www.youtube.com/playlist?list=PLsiFbTU1f8FBAoJCLgU7Ss9g_EK_Dhn3r");
    playlists.insert("rs2014", "https://www.youtube.com/playlist?list=PLEMIbGkCIAqB0U-5-lOFTKj8_drNJSayN");

    let url = playlists.get(query).unwrap_or(&query);

    let processing_msg = msg.channel_id.say(&ctx.http, "⏳ Processing playlist... This may take a moment.").await?;

    // TODO: Implement playlist processing using yt-dlp
    // This is a simplified version
    processing_msg.edit(&ctx.http, "Playlist processing not yet implemented in Rust version").await?;

    Ok(())
}

#[command]
#[required_permissions("ADMINISTRATOR")]
async fn setprefix(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let new_prefix = match args.single::<String>() {
        Ok(prefix) => prefix,
        Err(_) => {
            msg.channel_id.say(&ctx.http, "Please provide a new prefix").await?;
            return Ok(());
        }
    };

    let guild_id = msg.guild_id.unwrap();
    let old_prefix = GUILD_PREFIXES.get(&guild_id).map(|p| p.clone()).unwrap_or_else(|| "!".to_string());
    
    GUILD_PREFIXES.insert(guild_id, new_prefix.clone());
    
    if let Err(e) = save_guild_prefix(guild_id, &new_prefix).await {
        error!("Failed to save prefix: {}", e);
        msg.channel_id.say(&ctx.http, "Failed to save prefix").await?;
        return Ok(());
    }

    msg.channel_id.say(&ctx.http, format!("Prefix changed from `{}` to `{}`", old_prefix, new_prefix)).await?;

    Ok(())
}

#[command("help")]
async fn help_cmd(ctx: &Context, msg: &Message) -> CommandResult {
    let embed = CreateEmbed::new()
        .title("🎵 GoonBot Commands 🤤")
        .description("A Rust-powered music bot")
        .field("!goon <query>", "Play a song from YouTube", false)
        .field("!search <query>", "Search and select from top 5 results", false)
        .field("!queue", "Show the current queue", false)
        .field("!skip", "Skip the current song", false)
        .field("!pause", "Pause the current song", false)
        .field("!resume", "Resume the paused song", false)
        .field("!stop", "Stop playback and clear queue", false)
        .field("!shuffle", "Shuffle the queue", false)
        .field("!playlist <name>", "Play a predefined playlist", false)
        .field("!leave", "Leave the voice channel", false)
        .field("!setprefix <prefix>", "Set bot prefix (Admin only)", false)
        .color(Colour::BLURPLE)
        .footer(CreateEmbedFooter::new(format!("Build date: {}", BUILD_DATE)));

    msg.channel_id.send_message(&ctx.http, CreateMessage::new().embed(embed)).await?;

    Ok(())
}
