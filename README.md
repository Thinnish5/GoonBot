# Discord Music Bot - TypeScript

A feature-rich Discord music bot built with TypeScript and Discord.js that plays music from YouTube URLs.

## Features

- ✅ Play music from YouTube URLs
- ✅ Queue management (add, skip, view queue)
- ✅ Pause/Resume functionality
- ✅ Stop and clear queue
- ✅ Auto-disconnect when alone in voice channel
- ✅ Beautiful embeds for song information
- ✅ Fully typed with TypeScript

## Prerequisites

- Node.js 16.9.0 or higher
- npm or yarn
- Discord bot token
- ffmpeg (for audio processing)

### Install ffmpeg

**macOS:**
```bash
brew install ffmpeg
```

**Ubuntu/Debian:**
```bash
sudo apt-get install ffmpeg
```

**Windows:**
Download from [ffmpeg.org](https://ffmpeg.org/download.html)

## Setup

### 1. Clone or setup the project

```bash
cd /Users/diogo/Projetos/goonbot
```

### 2. Install dependencies

```bash
npm install
```

### 3. Create Discord Bot

1. Go to [Discord Developer Portal](https://discord.com/developers/applications)
2. Click "New Application"
3. Go to "Bot" tab and click "Add Bot"
4. Copy your bot token
5. Under "OAuth2 > URL Generator", select:
   - Scopes: `bot`
   - Permissions: `Send Messages`, `Use Slash Commands`, `Connect`, `Speak`
6. Copy the generated URL and open it to invite bot to your server

### 4. Configure environment

```bash
cp .env.example .env
```

Edit `.env` with your values:

```env
DISCORD_TOKEN=your_bot_token_here
CLIENT_ID=your_client_id_here
GUILD_ID=your_guild_id_here  # Optional: for testing, commands register faster
```

Find your IDs:
- `CLIENT_ID`: In Developer Portal > General Information
- `GUILD_ID`: Enable Developer Mode in Discord (User Settings > Advanced > Developer Mode), right-click server, copy ID

### 5. Build and run

**Development:**
```bash
npm run dev
```

**Production:**
```bash
npm run build
npm start
```

**Watch mode:**
```bash
npm run watch
```

## Commands

### `/play <url>`
Add a YouTube song to the queue and start playing

```
/play https://www.youtube.com/watch?v=dQw4w9WgXcQ
```

### `/queue`
View current queue and now playing song

### `/skip`
Skip the currently playing song

### `/pause`
Pause the current song

### `/resume`
Resume the paused song

### `/stop`
Stop playing and clear the queue

## Project Structure

```
goonbot/
├── src/
│   ├── index.ts           # Main bot file
│   ├── commands/          # Slash command handlers
│   │   ├── play.ts
│   │   ├── queue.ts
│   │   ├── skip.ts
│   │   ├── pause.ts
│   │   ├── resume.ts
│   │   └── stop.ts
│   ├── utils/             # Utility modules
│   │   ├── musicPlayer.ts # Audio playback
│   │   ├── queueManager.ts # Queue management
│   │   └── youtubeUtil.ts # YouTube helpers
│   └── types/             # TypeScript interfaces
│       └── index.ts
├── dist/                  # Compiled JavaScript
├── package.json
├── tsconfig.json
└── .env
```

## How It Works

1. **Queue System**: Each guild has its own queue managed by `QueueManager`
2. **Music Player**: `MusicPlayer` handles audio playback via Discord.js voice
3. **YouTube Integration**: `YouTubeUtil` fetches video info and streams audio
4. **Commands**: Slash commands handle user interactions
5. **Auto-Skip**: When a song finishes, the next song plays automatically

## Troubleshooting

### Bot doesn't respond to commands
- Check if bot has "Use Slash Commands" permission in the channel
- Ensure `DISCORD_TOKEN` and `CLIENT_ID` are correct
- Verify bot is in the Discord server

### No audio playing
- Ensure ffmpeg is installed: `ffmpeg -version`
- Check if bot has "Connect" and "Speak" permissions
- Verify you're in a voice channel before using `/play`

### YouTube URL not working
- Make sure the URL is valid and the video is accessible
- Bot cannot play age-restricted or private videos
- Check console for error messages

## Performance Tips

- The bot can handle multiple servers simultaneously
- Each guild has its own queue and music player instance
- Audio is streamed directly from YouTube (no local caching)

## Dependencies

- **discord.js**: Discord API wrapper
- **@discordjs/voice**: Voice channel support
- **ytdl-core**: YouTube audio streaming
- **typescript**: Type safety
- **dotenv**: Environment configuration

## License

MIT

## Notes

- This bot uses ytdl-core for YouTube streaming
- Ensure you comply with YouTube's Terms of Service
- The bot auto-disconnects when the last human leaves the channel
