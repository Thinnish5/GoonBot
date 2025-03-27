# 1st party modules
import asyncio

# 3rd party modules
from discord import FFmpegPCMAudio, PCMVolumeTransformer
import yt_dlp

ffmpeg_options = {
    "options": "-vn -b:a 192k -af loudnorm",
    "before_options": "-reconnect 1 -reconnect_streamed 1 -reconnect_delay_max 10 -nostdin -analyzeduration 2000000 -probesize 1000000",
}

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

playlist_ytdl_options = ytdl_format_options.copy().update(
    {
        "extract_flat": "in_playlist",  # Better playlist extraction
        "ignoreerrors": True,  # Skip failed entries
        "playlistend": 50,  # Limit to 50 items to avoid overloading
        "noplaylist": False,  # Allow playlist processing
    }
)

# Suppress noise about console usage from youtube_dl
yt_dlp.utils.bug_reports_message = lambda: ""

ytdl = yt_dlp.YoutubeDL(ytdl_format_options)

playlist_ytdl = yt_dlp.YoutubeDL(playlist_ytdl_options)


class YTDLSource(PCMVolumeTransformer):
    """Improved YouTube DL extractor with retry mechanism"""

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
                ytdl_instance = yt_dlp.YoutubeDL(ytdl_format_options)

                # Extract info
                data = await loop.run_in_executor(None, lambda: ytdl_instance.extract_info(url, download=not stream))

                if "entries" in data:
                    # Get the first item if it's a playlist
                    data = data["entries"][0]

                if not data:
                    continue

                filename = data["url"] if stream else ytdl_instance.prepare_filename(data)
                return cls(FFmpegPCMAudio(filename, **ffmpeg_options), data=data)

            except Exception as e:
                print(f"Stream extraction attempt {attempt+1} failed: {e}")
                if attempt == 2:  # Last attempt failed
                    raise

        raise Exception("All extraction attempts failed")
