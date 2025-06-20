use serenity::prelude::*;
use serenity::model::prelude::*;
use songbird::{input::Input, SerenityInit};
use std::sync::Arc;
use dashmap::DashMap;
use std::collections::VecDeque;
use once_cell::sync::Lazy;

pub struct Song {
    pub url: String,
    pub title: String,
    pub requested_by: UserId,
}

static QUEUES: Lazy<DashMap<GuildId, VecDeque<Song>>> = Lazy::new(DashMap::new);

pub async fn play_youtube(ctx: &Context, msg: &Message, query: &str) -> anyhow::Result<()> {
    let guild_id = msg.guild_id.ok_or_else(|| anyhow::anyhow!("Not in guild"))?;
    let manager = songbird::get(ctx).await.unwrap().clone();

    let channel_id = msg.author.voice(&ctx.cache).and_then(|v| v.channel_id);
    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            msg.reply(ctx, "You are not in a voice channel!").await?;
            return Ok(());
        }
    };

    let (_handler_lock, success) = manager.join(guild_id, connect_to).await;

    if let Ok(_conn_info) = success {
        // Detect if query is a YouTube link
        let is_url = query.contains("youtube.com") || query.contains("youtu.be");
        let search_term = if is_url {
            query.to_string()
        } else {
            format!("ytsearch1:{}", query)
        };

        // Use yt-dlp to get the direct audio URL and title
        let output = tokio::process::Command::new("yt-dlp")
            .arg("-f")
            .arg("bestaudio")
            .arg("-g")
            .arg(&search_term)
            .output()
            .await?;

        let url = String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or("").to_string();

        // Get title
        let title_output = tokio::process::Command::new("yt-dlp")
            .arg("--get-title")
            .arg(&search_term)
            .output()
            .await?;
        let title = String::from_utf8_lossy(&title_output.stdout).lines().next().unwrap_or("Unknown Title").to_string();

        if url.is_empty() {
            msg.reply(ctx, "Could not extract audio URL.").await?;
            return Ok(());
        }

        let song = Song {
            url: url.clone(),
            title: title.clone(),
            requested_by: msg.author.id,
        };

        let mut queue = QUEUES.entry(guild_id).or_insert_with(VecDeque::new);
        let position = queue.len() + 1;
        let should_start = queue.is_empty();
        queue.push_back(song);

        msg.reply(ctx, format!("Added to queue at position {}: `{}`", position, title)).await?;

        if should_start {
            play_next(ctx, guild_id).await?;
        }
    } else {
        msg.reply(ctx, "Failed to join voice channel.").await?;
    }
    Ok(())
}

async fn play_next(ctx: &Context, guild_id: GuildId) -> anyhow::Result<()> {
    let mut queue = QUEUES.entry(guild_id).or_insert_with(VecDeque::new);
    if let Some(song) = queue.pop_front() {
        let manager = songbird::get(ctx).await.unwrap().clone();
        if let Some(handler_lock) = manager.get(guild_id) {
            let mut handler = handler_lock.lock().await;
            let source = songbird::input::ffmpeg_args(
                song.url,
                &[
                    "-af",
                    "loudnorm=I=-16:TP=-1.5:LRA=11",
                ],
            )
            .await?;
            handler.enqueue_source(source);

            // Set up a handler to play the next song when this one ends
            let ctx = ctx.clone();
            tokio::spawn(async move {
                handler.add_global_event(
                    songbird::Event::Track(songbird::TrackEvent::End),
                    songbird::event::EventHandler::from(move |ctx: &songbird::EventContext<'_>| {
                        let ctx = ctx.clone();
                        let guild_id = guild_id;
                        Box::pin(async move {
                            let _ = play_next(&ctx, guild_id).await;
                        })
                    }),
                );
            });
        }
    }
    Ok(())
}

pub async fn show_queue(ctx: &Context, msg: &Message) -> anyhow::Result<()> {
    let guild_id = msg.guild_id.ok_or_else(|| anyhow::anyhow!("Not in guild"))?;
    let queue = QUEUES.get(&guild_id);

    if let Some(queue) = queue {
        if queue.is_empty() {
            msg.reply(ctx, "The queue is empty.").await?;
        } else {
            let list = queue
                .iter()
                .enumerate()
                .map(|(i, song)| format!("{}. {} (requested by <@{}>)", i + 1, song.title, song.requested_by))
                .collect::<Vec<_>>()
                .join("\n");
            msg.reply(ctx, format!("Current queue:\n{}", list)).await?;
        }
    } else {
        msg.reply(ctx, "The queue is empty.").await?;
    }
    Ok(())
}

pub async fn skip(ctx: &Context, msg: &Message) -> anyhow::Result<()> {
    let guild_id = msg.guild_id.ok_or_else(|| anyhow::anyhow!("Not in guild"))?;
    let manager = songbird::get(ctx).await.unwrap().clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        if handler.queue().is_empty() {
            msg.reply(ctx, "Nothing to skip!").await?;
        } else {
            handler.queue().skip(1)?;
            msg.reply(ctx, "Skipped to the next song!").await?;
        }
    } else {
        msg.reply(ctx, "Not in a voice channel!").await?;
    }
    Ok(())
}