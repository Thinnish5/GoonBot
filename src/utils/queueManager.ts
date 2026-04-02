import { Song, GuildQueue } from '../types/index';

export class QueueManager {
  private queues: Map<string, GuildQueue> = new Map();

  getQueue(guildId: string): GuildQueue {
    if (!this.queues.has(guildId)) {
      this.queues.set(guildId, {
        songs: [],
        playing: false,
      });
    }
    return this.queues.get(guildId)!;
  }

  addSong(guildId: string, song: Song): void {
    const queue = this.getQueue(guildId);
    queue.songs.push(song);
    console.log(`[QUEUE] Added song to ${guildId}: ${song.title} (url: ${song.url ? 'present' : 'missing'})`);
  }

  removeSong(guildId: string, index: number): Song | null {
    const queue = this.getQueue(guildId);
    if (index >= 0 && index < queue.songs.length) {
      const removed = queue.songs.splice(index, 1);
      return removed[0];
    }
    return null;
  }

  getCurrentSong(guildId: string): Song | undefined {
    const queue = this.getQueue(guildId);
    return queue.currentSong;
  }

  setCurrentSong(guildId: string, song: Song | undefined): void {
    const queue = this.getQueue(guildId);
    queue.currentSong = song;
  }

  getNextSong(guildId: string): Song | undefined {
    const queue = this.getQueue(guildId);
    return queue.songs.length > 0 ? queue.songs[0] : undefined;
  }

  dequeueNextSong(guildId: string): Song | undefined {
    const queue = this.getQueue(guildId);
    const nextSong = queue.songs.shift();
    console.log(`[QUEUE] Dequeued song from ${guildId}: ${nextSong?.title} (url: ${nextSong?.url ? 'present' : 'missing'})`);
    return nextSong;
  }

  skipSong(guildId: string): Song | undefined {
    return this.dequeueNextSong(guildId);
  }

  getAllSongs(guildId: string): Song[] {
    const queue = this.getQueue(guildId);
    return queue.songs;
  }

  clearQueue(guildId: string): void {
    const queue = this.getQueue(guildId);
    queue.songs = [];
    queue.currentSong = undefined;
    queue.playing = false;
  }

  setPlaying(guildId: string, playing: boolean): void {
    const queue = this.getQueue(guildId);
    queue.playing = playing;
  }

  isPlaying(guildId: string): boolean {
    const queue = this.getQueue(guildId);
    return queue.playing;
  }

  getQueueSize(guildId: string): number {
    const queue = this.getQueue(guildId);
    return queue.songs.length;
  }

  setPlayerMessage(guildId: string, messageId: string, channelId: string): void {
    const queue = this.getQueue(guildId);
    queue.playerMessageId = messageId;
    queue.playerChannelId = channelId;
  }

  getPlayerMessage(guildId: string): { messageId?: string; channelId?: string } {
    const queue = this.getQueue(guildId);
    return {
      messageId: queue.playerMessageId,
      channelId: queue.playerChannelId,
    };
  }

  setSongStartTime(guildId: string, startTime: number): void {
    const queue = this.getQueue(guildId);
    queue.songStartTime = startTime;
  }

  getSongElapsedTime(guildId: string): number {
    const queue = this.getQueue(guildId);
    if (!queue.songStartTime) return 0;
    return Math.floor((Date.now() - queue.songStartTime) / 1000);
  }
}
