import { execFile } from 'child_process';
import { promisify } from 'util';
import { Song } from '../types/index';

const execFileAsync = promisify(execFile);

export class YouTubeUtil {
  static async isValidUrl(url: string): Promise<boolean> {
    try {
      const parsed = new URL(url);
      const host = parsed.hostname.toLowerCase();
      return host === 'youtube.com' || host === 'www.youtube.com' || host === 'youtu.be' || host.endsWith('.youtube.com');
    } catch {
      return false;
    }
  }

  static async fetchSongInfo(url: string, userId: string): Promise<Song> {
    try {
      const { stdout } = await execFileAsync('yt-dlp', [
        '--dump-single-json',
        '--no-playlist',
        '--extractor-args',
        'youtube:player_client=web_creator',
        url,
      ]);

      const info = JSON.parse(stdout);

      return {
        id: String(info.id || 'unknown'),
        title: String(info.title || 'Unknown Title'),
        url: String(info.webpage_url || url),
        duration: Number(info.duration || 0),
        thumbnail: typeof info.thumbnail === 'string' ? info.thumbnail : undefined,
        addedBy: userId,
      };
    } catch (error) {
      console.error('Error fetching YouTube info:', error);
      // Fallback metadata keeps playback possible even when metadata extraction fails.
      return {
        id: 'unknown',
        title: 'YouTube Track',
        url,
        duration: 0,
        addedBy: userId,
      };
    }
  }

  static formatDuration(seconds: number): string {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;

    if (hours > 0) {
      return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    }
    return `${minutes}:${secs.toString().padStart(2, '0')}`;
  }
}
