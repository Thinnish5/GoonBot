export interface Song {
  id: string;
  title: string;
  url: string;
  duration: number;
  thumbnail?: string;
  addedBy: string;
}

export interface GuildQueue {
  songs: Song[];
  playing: boolean;
  currentSong?: Song;
  voiceConnection?: any;
  audioPlayer?: any;
  playerMessageId?: string;
  playerChannelId?: string;
  songStartTime?: number;
}
