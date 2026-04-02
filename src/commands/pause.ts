import { SlashCommandBuilder, ChatInputCommandInteraction, EmbedBuilder } from 'discord.js';
import { QueueManager } from '../utils/queueManager';
import { MusicPlayer } from '../utils/musicPlayer';

export const data = new SlashCommandBuilder()
  .setName('pause')
  .setDescription('Pause the current song');

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

  musicPlayer.pause();

  const embed = new EmbedBuilder()
    .setColor('#FF0000')
    .setTitle('⏸️ Paused')
    .setDescription(currentSong.title)
    .setFooter({ text: 'Use /resume to continue' })
    .setTimestamp();

  await interaction.reply({ embeds: [embed] });
}
