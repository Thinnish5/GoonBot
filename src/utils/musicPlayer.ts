import { AudioPlayer, AudioPlayerStatus, StreamType, VoiceConnection, createAudioPlayer, createAudioResource } from '@discordjs/voice';
import { Song } from '../types/index';
import { ChildProcess, execFile, spawn } from 'child_process';
import { PassThrough } from 'stream';
import { promisify } from 'util';

const execFileAsync = promisify(execFile);

export class MusicPlayer {
  private audioPlayer: AudioPlayer;
  private ffmpegProcess?: ChildProcess;

  constructor() {
    this.audioPlayer = createAudioPlayer();
    this.audioPlayer.on('error', (error) => {
      console.error('[audio-player] Error:', error.message);
    });
  }

  async playSong(voiceConnection: VoiceConnection, song: Song): Promise<void> {
    try {
      this.cleanupProcesses();
      const streamData = await this.getAudioStream(song);
      const resource = createAudioResource(streamData, {
        inputType: StreamType.OggOpus,
      });

      this.audioPlayer.play(resource);
      voiceConnection.subscribe(this.audioPlayer);
    } catch (error) {
      console.error(`Error playing song ${song.title}:`, error);
      throw error;
    }
  }

  private async resolveStreamUrl(url: string): Promise<string> {
    const { stdout } = await execFileAsync('yt-dlp', [
      '-f',
      'bestaudio',
      '-g',
      '--no-playlist',
      '--extractor-args',
      'youtube:player_client=web_creator',
      url,
    ]);

    const streamUrl = stdout
      .split('\n')
      .map((line) => line.trim())
      .find((line) => line.length > 0);

    if (!streamUrl) {
      throw new Error('yt-dlp did not return a direct stream URL');
    }

    return streamUrl;
  }

  private async getAudioStream(song: Song) {
    const streamUrl = await this.resolveStreamUrl(song.url);

    this.ffmpegProcess = spawn(
      'ffmpeg',
      [
        '-hide_banner',
        '-loglevel',
        'error',
        '-i',
        streamUrl,
        '-vn',
        '-acodec',
        'libopus',
        '-f',
        'ogg',
        'pipe:1',
      ],
      { stdio: ['ignore', 'pipe', 'pipe'] }
    );

    if (!this.ffmpegProcess.stdout) {
      throw new Error('Failed to initialize ffmpeg output stream');
    }

    if (this.ffmpegProcess.stderr) {
      this.ffmpegProcess.stderr.on('data', (chunk) => {
        const msg = chunk.toString().trim();
        if (msg) console.error('[ffmpeg]', msg);
      });
    }

    this.ffmpegProcess.on('error', (err) => {
      console.error('Error starting ffmpeg:', err);
    });

    const buffer = new PassThrough({ highWaterMark: 1024 * 1024 });
    this.ffmpegProcess.stdout.pipe(buffer);

    return buffer;
  }

  private cleanupProcesses(): void {
    if (this.ffmpegProcess && !this.ffmpegProcess.killed) {
      this.ffmpegProcess.kill('SIGKILL');
    }
    this.ffmpegProcess = undefined;
  }

  stop(): void {
    this.audioPlayer.stop();
    this.cleanupProcesses();
  }

  pause(): void {
    this.audioPlayer.pause();
  }

  unpause(): void {
    this.audioPlayer.unpause();
  }

  getPlayer(): AudioPlayer {
    return this.audioPlayer;
  }

  isPlaying(): boolean {
    return this.audioPlayer.state.status === AudioPlayerStatus.Playing;
  }
}
