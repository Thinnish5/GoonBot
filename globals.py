"""
Holds all the global constants used in the project
"""

BUILD_DATE = "2025-03-27"
DATA_DIR = "data"
DEFAULT_PREFIX = "!"
FFMPEG_BEFORE_OPTIONS = "-reconnect 1 -reconnect_streamed 1 -reconnect_delay_max 10 -nostdin -analyzeduration 2000000 -probesize 1000000"
FFMPEG_OPTIONS = "-reconnect 1 -reconnect_streamed 1 -reconnect_delay_max 10 -nostdin -analyzeduration 2000000 -probesize 1000000"
PREFIX_PATH = f"{DATA_DIR}/prefixes.json"
SECRET_FILE = "secret.secret"
