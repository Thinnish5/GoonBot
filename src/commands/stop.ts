import { SlashCommandBuilder, ChatInputCommandInteraction, EmbedBuilder } from 'discord.js';
import { QueueManager } from '../utils/queueManager';
import { MusicPlayer } from '../utils/musicPlayer';

export const data = new SlashCommandBuilder()
  .setName('stop')
  .setDescription('Stop music and clear the queue');

export async function execute(
  interaction: ChatInputCommandInteraction,
  queueManager: QueueManager,
  musicPlayer: MusicPlayer,
  _startPlayerUpdate: (guildId: string) => void,
  _cleanupOldMessages: (channelId: string) => Promise<void>
): Promise<void> {
  const guildId = interaction.guildId!;
  const isPlaying = queueManager.isPlaying(guildId);

  if (!isPlaying) {
    await interaction.reply("❌ Nothing is currently playing");
    return;
  }

  // Stop music and clear queue
  const queue = queueManager.getQueue(guildId);
  musicPlayer.stop();
  queue.voiceConnection?.destroy();
  queueManager.clearQueue(guildId);

  const embed = new EmbedBuilder()
    .setColor('#FF0000')
    .setTitle('⏹️ Stopped')
    .setDescription('Music stopped and queue cleared')
    .setFooter({ text: 'Stopped by ' + interaction.user.username })
    .setTimestamp();

  await interaction.reply({ embeds: [embed] });
}
