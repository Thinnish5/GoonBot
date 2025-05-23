# GoonBot

GoonBot is a Discord music bot that allows users to play music from YouTube directly in their voice channels. It supports commands for joining, leaving, playing, pausing, resuming, stopping, and skipping songs.


## Features

- **Play Music**: Play music from YouTube using `!goon <query>` - works with both URLs and search terms
- **Search & Queue**: Search for songs with `!goon search <query>` and select from top 5 results
- **Queue Management**: View the current queue with `!goon queue`
- **Playlist Support**: Play entire YouTube playlists with `!goon playlist <name or link>`
- **Controls**: Skip, pause, resume, and stop playback with intuitive commands and buttons
- **Auto-Disconnect**: Bot automatically leaves when everyone else leaves the voice channel
- **Shuffle**: Randomize your queue with `!goon shuffle`
- **Progress Bar**: Visual progress tracking for currently playing songs
- **High-Quality Audio**: Uses FFmpeg for high-quality audio streaming
- **Audio Normalization**: Ensures consistent volume levels across tracks


## Prerequisites

- Python (with pip) 3.11 or higher (3.13 is recomended)
- virtualenv (`python -m pip install virtualenv`)
- FFmpeg installed on your system (https://ffmpeg.org/download.html)
- A Discord bot token from the [Discord Developer Portal](https://discord.com/developers/applications).


## Running Locally

### Linux/macOs venv setup

Create the `secret.secret` file with your bot token.

```shell
# virtual environment to isolate the dependencies
virtualenv .venv
source .venv/bin/activate

# install bot dependencies
pip install -r requirements.txt

# launch the bot locally
python goon.py
```

## Git Hooks

To configure the git hooks run:
```shell
git config --local core.hooksPath .githooks
```

## References

### APIs & Libraries
- [Discord.py Documentation](https://discordpy.readthedocs.io/en/stable/)
- [YouTube Data API](https://developers.google.com/youtube/v3)
- [yt-dlp Documentation](https://github.com/yt-dlp/yt-dlp#readme)
- [FFmpeg Documentation](https://ffmpeg.org/documentation.html)
- [PyNaCl](https://pynacl.readthedocs.io/en/latest/)

### Helpful Resources
- [Discord Developer Portal](https://discord.com/developers/docs/intro)
- [Discord Bot Best Practices](https://discord.com/developers/docs/topics/community-resources#bots)
- [YouTube Terms of Service](https://www.youtube.com/t/terms)
- [Discord.py Voice Examples](https://github.com/Rapptz/discord.py/blob/master/examples/basic_voice.py)


## License
This project is licensed under the MIT License. See the [LICENSE](https://github.com/Thinnish5/goonbot/blob/main/LICENSE) file for details.
