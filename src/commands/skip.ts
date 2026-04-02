import { SlashCommandBuilder, ChatInputCommandInteraction, EmbedBuilder } from 'discord.js';
import { QueueManager } from '../utils/queueManager';
import { MusicPlayer } from '../utils/musicPlayer';

export const data = new SlashCommandBuilder()
  .setName('skip')
  .setDescription('Skip the current song');

export async function execute(
  interaction: ChatInputCommandInteraction,
  queueManager: QueueManager,
  musicPlayer: MusicPlayer,
  _startPlayerUpdate: (guildId: string) => void,
  _cleanupOldMessages: (channelId: string) => Promise<void>
): Promise<void> {
  const guildId = interaction.guildId!;
  const currentSong = queueManager.getCurrentSong(guildId);

  if (!currentSong) {
    await interaction.reply("❌ Nothing is currently playing");
    return;
  }

  // Skip current song
  const skipped = currentSong;
  const nextSong = queueManager.dequeueNextSong(guildId);
  queueManager.setCurrentSong(guildId, undefined);

  const embed = new EmbedBuilder()
    .setColor('#FF0000')
    .setTitle('⏭️ Skipped')
    .setDescription(skipped.title)
    .setFooter({ text: 'Skipped by ' + interaction.user.username })
    .setTimestamp();

  await interaction.reply({ embeds: [embed] });

  // Play next song if available
  if (nextSong) {
    const queue = queueManager.getQueue(guildId);
    if (queue.voiceConnection) {
      queueManager.setCurrentSong(guildId, nextSong);
      queueManager.setPlaying(guildId, true);
      await musicPlayer.playSong(queue.voiceConnection, nextSong);
    }
  } else {
    musicPlayer.stop();
    queueManager.setPlaying(guildId, false);
  }
}
