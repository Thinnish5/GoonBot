# 1st party modules
import asyncio
import json
from pathlib import Path
import random
import time

# 3rd party modules
import discord
from discord.ext import commands
from discord.ext import tasks
import yt_dlp as youtube_dl

# internal modules
from globals import *


# Function to read the bot token from secret.secret
def read_token():
    with open(SECRET_FILE, "r") as file:
        return file.read().strip()


# setup helpers
async def get_guild_prefix(guid_id: int):
    try:
        with open(PREFIX_PATH, "r") as f:
            prefixes = json.load(f)
            return prefixes.get(str(guid_id), DEFAULT_PREFIX)
    except (FileNotFoundError, json.decoder.JSONDecodeError):
        Path(DATA_DIR).mkdir(parents=True, exist_ok=True)
        with open(PREFIX_PATH, "w") as f:
            json.dump({}, f)
        return DEFAULT_PREFIX


async def get_prefix(bot, message: discord.Message):
    prefix = await get_guild_prefix(message.guild.id)
    return commands.when_mentioned_or(prefix)(bot, message)


# Bot setup
intents = discord.Intents.default()
intents.message_content = True
bot = commands.Bot(command_prefix=get_prefix, intents=intents)

# Suppress noise about console usage from youtube_dl
youtube_dl.utils.bug_reports_message = lambda: ""

# Update these options to use more reliable formats
ytdl_format_options = {
    # Prefer more stable formats
    "format": "bestaudio[ext=m4a]/bestaudio/best",
    "outtmpl": "%(extractor)s-%(id)s-%(title)s.%(ext)s",
    "restrictfilenames": True,
    "noplaylist": True,
    "nocheckcertificate": True,
    "ignoreerrors": False,
    "logtostderr": False,
    "quiet": True,
    "no_warnings": True,
    "default_search": "auto",
    "source_address": "0.0.0.0",
    "extract_flat": True,
    "prefer_ffmpeg": True,
    "cachedir": False,  # Prevent stale URL caching
    "http_headers": {  # More realistic browser headers
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
    },
}

# Update FFmpeg options for better reliability
ffmpeg_options = {
    "options": "-vn -b:a 192k -af loudnorm",
    "before_options": "-reconnect 1 -reconnect_streamed 1 -reconnect_delay_max 10 -nostdin -analyzeduration 2000000 -probesize 1000000",
}

ytdl = youtube_dl.YoutubeDL(ytdl_format_options)

# Queue system
queue = []

# Store player message references for each guild
player_messages = {}

# Add a dictionary to track the channels where the player should be shown
player_channels = {}

# Add this missing variable for tracking current songs
current_songs = {}


# Create a UI class for the player controls
class MusicPlayerView(discord.ui.View):
    def __init__(self):
        super().__init__(timeout=None)  # Persistent view

    @discord.ui.button(label="⏯️ Play/Pause", style=discord.ButtonStyle.primary, custom_id="play_pause")
    async def play_pause_button(self, interaction: discord.Interaction, button: discord.ui.Button):
        ctx = await bot.get_context(interaction.message)
        voice_client = interaction.guild.voice_client
        guild_id = interaction.guild.id

        if voice_client:
            if voice_client.is_paused():
                voice_client.resume()
                # Update pause tracking for progress bar
                if guild_id in current_songs:
                    info = current_songs[guild_id]
                    if info.get("paused_at"):
                        # Add the pause duration to total pause time
                        info["total_pause_time"] += time.time() - info["paused_at"]
                        info["paused_at"] = None
                    info["playing"] = True
                await interaction.response.send_message("Resumed playback", ephemeral=True, delete_after=5)
            elif voice_client.is_playing():
                voice_client.pause()
                # Track when we paused
                if guild_id in current_songs:
                    current_songs[guild_id]["playing"] = False
                    current_songs[guild_id]["paused_at"] = time.time()
                await interaction.response.send_message("Paused playback", ephemeral=True, delete_after=5)
            else:
                await interaction.response.send_message("Nothing is playing", ephemeral=True, delete_after=5)

        # Update the player
        await update_player(ctx)

    @discord.ui.button(label="⏭️ Skip", style=discord.ButtonStyle.primary, custom_id="skip")
    async def skip_button(self, interaction: discord.Interaction, button: discord.ui.Button):
        ctx = await bot.get_context(interaction.message)
        voice_client = interaction.guild.voice_client

        if voice_client and voice_client.is_playing():
            voice_client.stop()
            await interaction.response.send_message("Skipped the current song", ephemeral=True, delete_after=5)
        else:
            await interaction.response.send_message("Nothing to skip", ephemeral=True, delete_after=5)

    @discord.ui.button(label="⏹️ Stop", style=discord.ButtonStyle.danger, custom_id="stop")
    async def stop_button(self, interaction: discord.Interaction, button: discord.ui.Button):
        ctx = await bot.get_context(interaction.message)
        voice_client = interaction.guild.voice_client

        if voice_client and (voice_client.is_playing() or voice_client.is_paused()):
            queue.clear()
            voice_client.stop()
            await interaction.response.send_message("Stopped playback and cleared the queue", ephemeral=True, delete_after=5)
        else:
            await interaction.response.send_message("Nothing is playing", ephemeral=True, delete_after=5)

        # Update the player
        await update_player(ctx)

    @discord.ui.button(label="📋 Show Queue", style=discord.ButtonStyle.secondary, custom_id="queue")
    async def queue_button(self, interaction: discord.Interaction, button: discord.ui.Button):
        # Defer the response immediately to prevent timeout
        await interaction.response.defer(ephemeral=True)

        # Then process the queue display (which might take time)
        queue_display = await get_queue_display()

        # Send followup instead of direct response
        await interaction.followup.send(queue_display, ephemeral=True)

    # Add this button to your MusicPlayerView class
    @discord.ui.button(label="🔀 Shuffle", style=discord.ButtonStyle.secondary, custom_id="shuffle")
    async def shuffle_button(self, interaction: discord.Interaction, button: discord.ui.Button):
        ctx = await bot.get_context(interaction.message)

        if len(queue) <= 1:
            await interaction.response.send_message("Need at least 2 songs in the queue to shuffle.", ephemeral=True)
            return

        random.shuffle(queue)
        await interaction.response.send_message("🔀 Queue has been shuffled!", ephemeral=True)

        # Update the player to show the new queue
        await update_player(ctx)


# Function to create or update the player message
# Update the update_player function to use stored song information
# Update the update_player function to show thumbnails
async def update_player(ctx, song_title=None, is_playing=False):
    guild_id = ctx.guild.id

    # Track this channel as having a player
    player_channels[guild_id] = ctx.channel.id

    # Create embed for player
    embed = discord.Embed(title="🎵 GoonBot Music Player 🤤", color=discord.Color.blurple())

    # Check if we have current song info (this takes priority)
    if guild_id in current_songs:
        info = current_songs[guild_id]
        song_title = info.get("title", "Unknown")
        is_playing = info.get("playing", False)

        # Add thumbnail if available
        if "thumbnail" in info and info["thumbnail"]:
            embed.set_thumbnail(url=info["thumbnail"])

        # Calculate current progress
        duration = info.get("duration", 0)
        if duration > 0:
            start_time = info.get("start_time", time.time())
            total_pause_time = info.get("total_pause_time", 0)

            # If currently paused, don't include time since pause
            if info.get("paused_at"):
                elapsed = info["paused_at"] - start_time - total_pause_time
            else:
                elapsed = time.time() - start_time - total_pause_time

            # Create progress display
            progress_bar = create_progress_bar(elapsed, duration)
            time_display = f"{format_time(elapsed)}/{format_time(duration)}"
        else:
            progress_bar = None
            time_display = None

    if song_title and is_playing:
        if progress_bar and time_display:
            embed.description = f"**Now Gooning:**\n{song_title}\n\n{progress_bar} {time_display}"
        else:
            embed.description = f"**Now Gooning:**\n{song_title}"
        embed.set_footer(text="Use the buttons below to control playback")
    elif len(queue) > 0:
        embed.description = f"**Up Next:** {queue[0]}\n*{len(queue)} songs in queue*"
        embed.set_footer(text="Nothing is currently gooning")
    else:
        embed.description = "Nothing is gooning right now"
        embed.set_footer(text="Use !goon <query> to add songs to the queue")

    # Rest of the function remains the same
    view = MusicPlayerView()

    # Try to update existing message first
    if guild_id in player_messages and player_messages[guild_id]:
        try:
            message = player_messages[guild_id]
            await message.edit(embed=embed, view=view)

            # Clean up any duplicate player messages
            await cleanup_previous_player_messages(ctx)
            return
        except discord.NotFound:
            # Message was deleted, create a new one
            pass

    # Clean up any old player messages before creating a new one
    await cleanup_previous_player_messages(ctx)

    # Create new message
    message = await ctx.send(embed=embed, view=view)
    player_messages[guild_id] = message


# Improved YouTube DL extractor with retry mechanism
class YTDLSource(discord.PCMVolumeTransformer):
    def __init__(self, source, *, data, volume=0.5):
        super().__init__(source, volume)
        self.data = data
        self.title = data.get("title", "Unknown title")
        self.url = data.get("url", "")

    @classmethod
    async def from_url(cls, url, *, loop=None, stream=False):
        loop = loop or asyncio.get_event_loop()

        # Try multiple times with different formats if needed
        for attempt in range(3):
            try:
                # Wait between retries
                if attempt > 0:
                    await asyncio.sleep(1)
                    print(f"Retry attempt {attempt} for {url}")

                # Create a fresh YTDL instance for each attempt
                ytdl_instance = youtube_dl.YoutubeDL(ytdl_format_options)

                # Extract info
                data = await loop.run_in_executor(None, lambda: ytdl_instance.extract_info(url, download=not stream))

                if "entries" in data:
                    # Get the first item if it's a playlist
                    data = data["entries"][0]

                if not data:
                    continue

                filename = data["url"] if stream else ytdl_instance.prepare_filename(data)
                return cls(discord.FFmpegPCMAudio(filename, **ffmpeg_options), data=data)

            except Exception as e:
                print(f"Stream extraction attempt {attempt+1} failed: {e}")
                if attempt == 2:  # Last attempt failed
                    raise

        raise Exception("All extraction attempts failed")


# Add this helper function to clean up previous player messages
async def cleanup_previous_player_messages(ctx):
    """Find and delete old player messages from the bot in the current channel"""
    try:
        # Get recent messages in the channel
        channel = ctx.channel
        messages = [msg async for msg in channel.history(limit=15)]

        # Filter for bot's messages that look like player messages
        player_messages_list = [
            msg
            for msg in messages
            if msg.author.id == bot.user.id and msg.embeds and any("GoonBot Music Player" in embed.title for embed in msg.embeds if embed.title)
        ]

        # If there's more than one, delete all except the most recent
        if len(player_messages_list) > 1:
            print(f"Found {len(player_messages_list)} player messages in {channel.name}, cleaning up...")
            # Skip the first one (most recent) if we have a current player reference
            for msg in player_messages_list[1:]:
                try:
                    await msg.delete()
                    print(f"Deleted old player message in {channel.name}")
                except Exception as e:
                    print(f"Error deleting old message: {e}")
    except Exception as e:
        print(f"Error in cleanup: {e}")


# Function to play the next song in the queue
async def play_next(ctx):
    if len(queue) > 0:
        query = queue[0]
        try:
            player = await YTDLSource.from_url(query, loop=bot.loop, stream=True)
            queue.pop(0)

            # Extract thumbnail URL from the data
            thumbnail_url = None
            if "thumbnail" in player.data:
                thumbnail_url = player.data["thumbnail"]
            # If no direct thumbnail, check thumbnails list
            elif "thumbnails" in player.data and len(player.data["thumbnails"]) > 0:
                # Try to get the highest quality thumbnail
                thumbnails = player.data["thumbnails"]
                # Usually the last one is highest quality
                thumbnail_url = thumbnails[-1]["url"] if thumbnails else None

            # Store enhanced song information
            guild_id = ctx.guild.id
            duration = player.data.get("duration", 0)
            current_songs[guild_id] = {
                "title": player.title,
                "playing": True,
                "start_time": time.time(),
                "duration": duration,
                "paused_at": None,
                "total_pause_time": 0,
                "thumbnail": thumbnail_url,  # Store the thumbnail URL
            }

            ctx.voice_client.play(
                player,
                after=lambda e: asyncio.run_coroutine_threadsafe(handle_playback_error(e, ctx), bot.loop),
            )

            # Update the player with the current song
            await update_player(ctx)
        except Exception as e:
            print(f"Error playing {query}: {e}")
            queue.pop(0)
            await ctx.send(f"❌ Error playing song. Skipping to next...", delete_after=5)
            await asyncio.sleep(1)
            await play_next(ctx)
    else:
        # Clear current song when queue is empty
        if ctx.guild.id in current_songs:
            del current_songs[ctx.guild.id]
        await update_player(ctx)


# Add this helper function to handle playback errors
async def handle_playback_error(error, ctx):
    if error:
        print(f"Playback error: {error}")
    await play_next(ctx)


# Create a command group for GoonBot
@bot.group(name="goon", invoke_without_command=True)
async def goon(ctx, *, query: str = None):
    if query is None:
        await ctx.send("Please provide a search query. Usage: `!goon <query>`")
        return

    # Join the voice channel if not already connected
    if not ctx.message.author.voice:
        await ctx.send(f"{ctx.message.author.name} is not connected to a voice channel", ephemeral=True, delete_after=5)
        return

    voice_client = ctx.message.guild.voice_client
    if not voice_client or not voice_client.is_connected():
        channel = ctx.message.author.voice.channel
        voice_client = await channel.connect()

    # Check if the query is a URL or a search term
    is_url = "youtube.com" in query or "youtu.be" in query

    if is_url:
        # Existing behavior for URLs
        actual_query = query
        temp_msg = await ctx.send(f"Added to queue: `{query}`")
    else:
        # New behavior for search terms: auto-select first result
        temp_msg = await ctx.send(f"🔍 Searching for: `{query}`...")

        try:
            # Use ytsearch1: to get just the top result
            data = await bot.loop.run_in_executor(None, lambda: ytdl.extract_info(f"ytsearch1:{query}", download=False))

            if "entries" not in data or len(data["entries"]) == 0:
                await temp_msg.edit(content="❌ No results found.")
                return

            # Get the first (and only) result
            result = data["entries"][0]
            actual_query = result["url"]

            # Update the message with the found song
            await temp_msg.edit(content=f"✅ Added to queue: `{result['title']}`")

        except Exception as e:
            print(f"Search error: {e}")
            await temp_msg.edit(content=f"❌ Error searching YouTube: {str(e)[:100]}...")
            return

    # Add the song to the queue (now using actual_query which is always a URL)
    queue.append(actual_query)

    # If nothing is playing, start playing the song
    if not voice_client.is_playing():
        await play_next(ctx)
    else:
        # Update the player to show the new queue
        await update_player(ctx)

    # Delete the temporary message after a few seconds
    await asyncio.sleep(5)
    try:
        await temp_msg.delete()
    except:
        pass


@goon.command(name="info", help="Shows information about the bot")
async def info(ctx):
    await ctx.send(f"Build date: {BUILD_DATE}")


# Command: !goon search <query>
@goon.command(name="search", help="Searches for a song and lets you select from the top 5 results")
async def search(ctx, *, query: str):
    if not query:
        await ctx.send("Please provide a search query. Usage: `!goon search <query>`")
        return

    # Join the voice channel if not already connected
    if not ctx.message.author.voice:
        await ctx.send(f"{ctx.message.author.name} is not connected to a voice channel", ephemeral=True, delete_after=5)
        return

    voice_client = ctx.message.guild.voice_client
    if not voice_client or not voice_client.is_connected():
        channel = ctx.message.author.voice.channel
        voice_client = await channel.connect()

    # Fetch the top 5 search results
    try:
        data = await bot.loop.run_in_executor(None, lambda: ytdl.extract_info(f"ytsearch5:{query}", download=False))
        if "entries" not in data or len(data["entries"]) == 0:
            await ctx.send("No results found.")
            return

        # Display the top 5 results
        results = data["entries"]
        message = "**Search Results:**\n"
        for i, entry in enumerate(results):
            message += f"{i + 1}. {entry['title']} - `{entry['url']}`\n"
        message += "\nReact with the number (1️⃣-5️⃣) of the song you want to play."

        # Send the results and add reactions
        sent_message = await ctx.send(message)
        for emoji in ["1️⃣", "2️⃣", "3️⃣", "4️⃣", "5️⃣"]:
            await sent_message.add_reaction(emoji)

        # Set up a task to delete the message after 30 seconds
        async def delete_after_delay():
            await asyncio.sleep(30)
            try:
                await sent_message.delete()
            except discord.NotFound:
                pass  # Message already deleted

        # Start the deletion task
        deletion_task = asyncio.create_task(delete_after_delay())

        # Wait for the user to react
        def check(reaction, user):
            return user == ctx.author and str(reaction.emoji) in [
                "1️⃣",
                "2️⃣",
                "3️⃣",
                "4️⃣",
                "5️⃣",
            ]

        try:
            reaction, _ = await bot.wait_for("reaction_add", timeout=30.0, check=check)
            # Cancel deletion task if user responded before the timeout
            deletion_task.cancel()

            selected_index = ["1️⃣", "2️⃣", "3️⃣", "4️⃣", "5️⃣"].index(str(reaction.emoji))
            selected_url = results[selected_index]["url"]

            # Add the selected song to the queue
            queue.append(selected_url)
            confirm_msg = await ctx.send(f"Added to queue: {results[selected_index]['title']}")

            # Delete the search results message immediately after selection
            await sent_message.delete()

            # Delete confirmation message after 5 seconds
            await asyncio.sleep(5)
            await confirm_msg.delete()

            # If nothing is playing, start playing the song
            if not voice_client.is_playing():
                await play_next(ctx)

        except asyncio.TimeoutError:
            # User didn't respond in time - message will be deleted by the deletion_task
            timeout_msg = await ctx.send("You took too long to select a song. Please try again.")
            await asyncio.sleep(5)
            await timeout_msg.delete()

    except Exception as e:
        print(f"Error: {e}")
        await ctx.send("An error occurred while processing the search query. Please try again.")


# Helper function to generate queue display
async def get_queue_display():
    if len(queue) == 0:
        return "The queue is empty."

    # Get up to 10 items from the queue
    display_queue = queue[:5]

    # Try to extract titles where possible
    queue_items = []
    for i, item in enumerate(display_queue):
        # Try to get a title from the URL
        try:
            # Only extract information if this is a direct URL
            if "youtube.com" in item or "youtu.be" in item:
                data = await asyncio.get_event_loop().run_in_executor(None, lambda: ytdl.extract_info(item, download=False, process=False))
                title = data.get("title", item)
            else:
                title = item
            queue_items.append(f"{i + 1}. `{title}`")
        except:
            # If we can't extract a title, just use the query string
            queue_items.append(f"{i + 1}. `{item}`")

    # Show how many more songs are in queue if there are more than 10
    remaining = len(queue) - 5
    queue_text = "\n".join(queue_items)

    if remaining > 0:
        queue_text += f"\n\n*...and {remaining} more song{'s' if remaining != 1 else ''}*"

    return f"**Current queue:**\n{queue_text}"


# Command: !goon queue
@goon.command(name="queue", help="Displays the current queue")
async def show_queue(ctx):
    queue_display = await get_queue_display()
    # Using ephemeral=True to make it visible only to the user
    await ctx.send(queue_display, ephemeral=True)


# Command: !goon skip
@goon.command(name="skip", help="Skips the current song")
async def skip(ctx):
    voice_client = ctx.message.guild.voice_client
    if voice_client.is_playing():
        voice_client.stop()
        await ctx.send("Skipped the current song.")
    else:
        await ctx.send("No audio is playing.")


# Command: !goon leave
@goon.command(name="leave", help="Leaves the voice channel")
async def leave(ctx):
    voice_client = ctx.message.guild.voice_client
    if voice_client and voice_client.is_connected:
        # Clear the player message before leaving
        guild_id = ctx.guild.id
        if guild_id in player_messages:
            try:
                old_message = player_messages[guild_id]
                await old_message.delete()
                player_messages.pop(guild_id, None)  # Remove from dictionary
                player_channels.pop(guild_id, None)  # Remove channel tracking
            except (discord.NotFound, AttributeError):
                pass

        # Now disconnect and clear queue
        await voice_client.disconnect()
        queue.clear()  # Clear the queue when the bot leaves
        await ctx.send("Left the voice channel and cleared the queue.", delete_after=5)
    else:
        await ctx.send("The bot is not connected to a voice channel.", delete_after=5)


# Command: !goon pause
@goon.command(name="pause", help="Pauses the current song")
async def pause(ctx):
    voice_client = ctx.message.guild.voice_client
    if voice_client.is_playing():
        voice_client.pause()
    else:
        await ctx.send("No audio is playing.")


# Command: !goon resume
@goon.command(name="resume", help="Resumes the paused song")
async def resume(ctx):
    voice_client = ctx.message.guild.voice_client
    if voice_client.is_paused():
        voice_client.resume()
    else:
        await ctx.send("The audio is not paused.")


# Command: !goon stop
@goon.command(name="stop", help="Stops the current song")
async def stop(ctx):
    voice_client = ctx.message.guild.voice_client
    if voice_client.is_playing():
        voice_client.stop()
        queue.clear()  # Clear the queue when the bot stops
    else:
        await ctx.send("No audio is playing.")


@goon.command(name="help", help="Displays the help message")
async def help(ctx):
    await ctx.send(
        "GoonBot is a music bot that can play songs from YouTube. Usage:\n"
        "1. `!goon <query>`: Plays the song with the given query.\n"
        "2. `!goon search <query>`: Searches for a song and lets you select from the top 5 results.\n"
        "3. `!goon queue`: Displays the current queue.\n"
        "4. `!goon skip`: Skips the current song.\n"
        "5. `!goon leave`: Leaves the voice channel.\n"
        "6. `!goon pause`: Pauses the current song.\n"
        "7. `!goon resume`: Resumes the paused song.\n"
        "8. `!goon stop`: Stops the current song.\n"
        "9. `!goon playlist <name or link>`: Plays a playlist by name or link.\n"
        "10.`!goon shuffle`: Shuffles the songs in the queue."
    )


# Command: !goon shuffle
@goon.command(name="shuffle", help="Shuffles the songs in the queue")
async def shuffle(ctx):
    if len(queue) <= 1:
        await ctx.send("Need at least 2 songs in the queue to shuffle.")
        return

    random.shuffle(queue)
    await ctx.send("🔀 Queue has been shuffled!")

    # Update the player to show the new queue order
    await update_player(ctx)


# Command: !goon playlist <name or link>
@goon.command(name="playlist", help="Plays a playlist by name or link")
async def playlist(ctx, *, query: str):
    playlists = {
        "all": "https://www.youtube.com/playlist?list=PLEMIbGkCIAqDaWDz1hwg04BwSbDXDlPCh",
        "good": "https://www.youtube.com/playlist?list=PLEMIbGkCIAqDObuOVPqYQCyaibK8aYsI3",
        "loop": "https://www.youtube.com/playlist?list=PLEMIbGkCIAqDBYM493VDLLfSlsgXeBURI",
        "emo": "https://www.youtube.com/playlist?list=PLEMIbGkCIAqA0koaFqXvRVsed5lAs7BLF",
        "dl": "https://www.youtube.com/playlist?list=PLEMIbGkCIAqDDeWgpUEK7gV5wtRwgckuI",
        "diogo": "https://www.youtube.com/playlist?list=PL0SNMtspDuGR8cS7KNdAUYK196TAom2iJ",
        "undead": "https://www.youtube.com/playlist?list=PLsiFbTU1f8FBAoJCLgU7Ss9g_EK_Dhn3r",
        "rs2014": "https://www.youtube.com/playlist?list=PLEMIbGkCIAqB0U-5-lOFTKj8_drNJSayN",
    }

    if query in playlists:
        url = playlists[query]
    else:
        # Extract playlist ID and convert to proper playlist URL
        import re

        # Check if this is a YouTube URL with a video and playlist ID
        playlist_id_match = re.search(r"list=([^&]+)", query)
        if playlist_id_match and ("youtube.com/watch" in query or "youtu.be" in query):
            # Extract the playlist ID
            playlist_id = playlist_id_match.group(1)
            # Convert to the proper format YouTube API prefers
            url = f"https://www.youtube.com/playlist?list={playlist_id}"
            print(f"Converted URL from {query} to {url}")
        else:
            url = query

    # Join the voice channel if not already connected
    if not ctx.message.author.voice:
        await ctx.send(f"{ctx.message.author.name} is not connected to a voice channel", ephemeral=True, delete_after=5)
        return

    voice_client = ctx.message.guild.voice_client
    if not voice_client or not voice_client.is_connected():
        channel = ctx.message.author.voice.channel
        voice_client = await channel.connect()

    # Let the user know we're processing the playlist
    processing_msg = await ctx.send("⏳ Processing playlist... This may take a moment.")

    # Create a custom ytdl instance with playlist-specific options
    playlist_ytdl_options = ytdl_format_options.copy()
    playlist_ytdl_options.update(
        {
            "extract_flat": "in_playlist",  # Better playlist extraction
            "ignoreerrors": True,  # Skip failed entries
            "playlistend": 50,  # Limit to 50 items to avoid overloading
            "noplaylist": False,  # Allow playlist processing
        }
    )
    playlist_ytdl = youtube_dl.YoutubeDL(playlist_ytdl_options)

    # Fetch the playlist items
    try:
        # First, get playlist info without downloading
        data = await bot.loop.run_in_executor(None, lambda: playlist_ytdl.extract_info(url, download=False, process=False))

        if "entries" not in data:
            await processing_msg.edit(content="No playlist found or invalid URL.")
            return

        # Process each entry individually to avoid batch errors
        successful_entries = 0
        entry_count = 0

        # Process the entries as they come in from the generator
        for entry in data["entries"]:
            entry_count += 1
            if entry is None:
                continue

            try:
                # For each entry, get a proper URL
                if "url" in entry and entry["url"]:
                    video_url = entry["url"]
                elif "id" in entry and entry["id"]:
                    video_url = f"https://www.youtube.com/watch?v={entry['id']}"
                else:
                    continue

                # Add to queue
                queue.append(video_url)
                successful_entries += 1

                # Update processing message periodically
                if successful_entries % 5 == 0:
                    await processing_msg.edit(content=f"⏳ Added {successful_entries} songs so far...")

            except Exception as e:
                print(f"Error processing playlist item: {e}")
                continue

        # Check if we processed any entries
        if entry_count == 0:
            await processing_msg.edit(content="The playlist appears to be empty.")
            return

        # Update final message
        if successful_entries > 0:
            await processing_msg.edit(content=f"✅ Added {successful_entries} songs to the queue.", delete_after=5)

            # If nothing is playing, start playing the first song
            if not voice_client.is_playing():
                await play_next(ctx)
            else:
                # Update the player to show the new queue
                await update_player(ctx)
        else:
            await processing_msg.edit(content="❌ Failed to add any songs from the playlist.")

    except Exception as e:
        print(f"Playlist error: {e}")
        await processing_msg.edit(content=f"❌ Error processing playlist: {str(e)[:100]}...")


@goon.command(name="setprefix", help="Allows to change the prefix of the bot in the guild")
@commands.has_permissions(administrator=True)
async def setprefix(ctx, prefix):
    with open(PREFIX_PATH, "r") as f:
        prefixes = json.load(f)
    current_prefix = prefixes.get(str(ctx.guild.id), DEFAULT_PREFIX)
    prefixes[str(ctx.guild.id)] = prefix
    with open(PREFIX_PATH, "w") as f:
        json.dump(prefixes, f, indent=2)
    await ctx.send(f"Prefix changed from {current_prefix} to {prefix}")


# Add this before bot.run(read_token())
@bot.event
async def on_ready():
    print(f"Bot is ready! Logged in as {bot.user}")
    # Register the persistent view
    bot.add_view(MusicPlayerView())
    # Start the progress update task
    update_progress_bars.start()


@bot.event
async def on_guild_join(guild):
    with open(PREFIX_PATH, "r") as f:
        prefixes = json.load(f)
    prefixes[str(guild.id)] = DEFAULT_PREFIX
    with open(PREFIX_PATH, "w") as f:
        json.dump(prefixes, f, indent=2)


@bot.event
async def on_guild_remove(guild):
    with open(PREFIX_PATH, "r") as f:
        prefixes = json.load(f)
    prefixes.pop(str(guild.id), None)
    with open(PREFIX_PATH, "w") as f:
        json.dump(prefixes, f, indent=2)


# Add this event to listen for new messages
@bot.event
async def on_message(message):
    # Don't process commands here, just watch for new messages
    if message.author == bot.user:
        return

    # Process commands first
    await bot.process_commands(message)

    # Check if this channel has a player
    guild_id = message.guild.id if message.guild else None
    if guild_id and guild_id in player_channels and player_channels[guild_id] == message.channel.id:
        # Get context for sending messages
        ctx = await bot.get_context(message)

        # Only update if the player exists
        if guild_id in player_messages:
            # Delete the old player message
            try:
                old_message = player_messages[guild_id]
                await old_message.delete()
            except (discord.NotFound, AttributeError):
                # Message might already be deleted
                pass
            # Create a new player message
            player_messages[guild_id] = None
            await update_player(ctx)


@bot.event
async def on_voice_state_update(member, before, after):
    # First handle the bot's own disconnects
    if member.id == bot.user.id:
        if before.channel is not None and after.channel is None:
            guild_id = before.channel.guild.id

            # Clean up the player message
            if guild_id in player_messages:
                try:
                    # Get the channel where the player is
                    channel_id = player_channels.get(guild_id)
                    if channel_id:
                        channel = bot.get_channel(channel_id)
                        if channel:
                            # Get the message and delete it
                            old_message = player_messages[guild_id]
                            await old_message.delete()

                    # Remove from dictionaries
                    player_messages.pop(guild_id, None)
                    player_channels.pop(guild_id, None)
                except (discord.NotFound, AttributeError, discord.HTTPException):
                    # Handle any errors during deletion
                    pass

        # handling for voice connection errors
        if hasattr(after, "channel") and after.channel is not None:
            if hasattr(after, "self_deaf") and after.self_deaf is True:
                print(f"Bot reconnected to voice in {after.channel.guild.name}")
        return

    # Now handle when a user disconnects from a voice channel
    if before.channel is not None and (after.channel is None or after.channel.id != before.channel.id):
        # Check if the bot is in this channel
        voice_client = before.channel.guild.voice_client
        if voice_client and voice_client.channel.id == before.channel.id:

            # Count remaining human users in the channel
            human_members = [m for m in before.channel.members if not m.bot and m.id != bot.user.id]

            # If no humans left, disconnect the bot
            # Improved disconnect handler in on_voice_state_update
            if len(human_members) == 0:
                print(f"All users left voice channel in {before.channel.guild.name} - disconnecting")

                guild_id = before.channel.guild.id

                # First, properly store message reference before cleanup
                old_message = None
                channel = None
                if guild_id in player_messages and player_messages[guild_id]:
                    old_message = player_messages[guild_id]

                    # Get the text channel reference
                    if guild_id in player_channels:
                        channel_id = player_channels.get(guild_id)
                        channel = bot.get_channel(channel_id)

                # Disconnect from voice first (stops any playback)
                await voice_client.disconnect()

                # Clear the queue and current song info
                if guild_id in current_songs:
                    del current_songs[guild_id]

                # Now delete the player message AFTER disconnecting
                if old_message and channel:
                    try:
                        await old_message.delete()
                        print(f"Successfully deleted player message in {channel.name}")
                    except Exception as e:
                        print(f"Error deleting player message: {e}")

                # Clean up references AFTER deletion attempt
                player_messages.pop(guild_id, None)
                player_channels.pop(guild_id, None)

                # Send notification if we have a valid channel
                if channel:
                    await channel.send("Everyone left the voice channel, so I disconnected.", delete_after=10)


# Add these helper functions to format time and create progress bar
def format_time(seconds):
    """Convert seconds to MM:SS format"""
    if seconds is None:
        return "0:00"
    minutes, seconds = divmod(int(seconds), 60)
    return f"{minutes}:{seconds:02d}"


def create_progress_bar(current, total, length=20):
    if total <= 0:
        return "┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈"

    current = min(current, total)
    percentage = current / total
    position = int(percentage * length)

    # More elegant look
    filled = "━" * position
    empty = "┈" * (length - position - 1)

    # Create bar with playhead
    bar = filled + "⚪" + empty

    return bar


# Add a task to update the player periodically
@tasks.loop(seconds=1)
async def update_progress_bars():
    """Update all active players to show current progress"""
    for guild_id, info in list(current_songs.items()):
        if info.get("playing", False) and info.get("duration", 0) > 0:
            # Only update if we have an active player
            if guild_id in player_messages and guild_id in player_channels:
                try:
                    channel_id = player_channels[guild_id]
                    channel = bot.get_channel(channel_id)
                    if channel and player_messages[guild_id]:
                        ctx = await bot.get_context(player_messages[guild_id])
                        await update_player(ctx)
                except Exception as e:
                    print(f"Error updating progress bar: {e}")


if __name__ == "__main__":
    bot.run(read_token())
