# GoonBot

GoonBot is a Discord music bot that allows users to play music from YouTube directly in their voice channels. It supports commands for joining, leaving, playing, pausing, resuming, stopping, and skipping songs.


## Features

- **Play Music**: Play music from YouTube using `!goon <YouTube Link>`.
- **Join/Leave Voice Channels**: Automatically join and leave voice channels.
- **Pause/Resume**: Pause and resume the current song.
- **Stop**: Stop the current song.
- **Skip**: Skip the current song.
- **High-Quality Audio**: Uses FFmpeg for high-quality audio streaming.
- **Audio Normalization**: Ensures consistent volume levels across tracks.


## Prerequisites

- Python 3.8 or higher.
- virtualenv (`python -m pip install virtualenv`)
- FFmpeg installed on your system.
- A Discord bot token from the [Discord Developer Portal](https://discord.com/developers/applications).


## Running Locally

### Windows venv setup

```shell
virtualenv .venv
.\.venv\Scripts\activate
```

### Linux/macOs venv setup

```shell
virtualenv .venv
source .venv/bin/activate
```

### Project setup

Create the `secret.secret` file with your bot token.

```shell
# install development dependencies (also installs production dependencies)
pip install -r requirements-dev.txt

# launch the bot locally
python goon.py
```


## License
This project is licensed under the MIT License. See the [LICENSE](https://github.com/Thinnish5/goonbot/blob/main/LICENSE) file for details.
