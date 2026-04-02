import { SlashCommandBuilder, ChatInputCommandInteraction, EmbedBuilder } from 'discord.js';
import { QueueManager } from '../utils/queueManager';
import { MusicPlayer } from '../utils/musicPlayer';

export const data = new SlashCommandBuilder()
  .setName('leave')
  .setDescription('Leave the voice channel and clear the queue');

export async function execute(
  interaction: ChatInputCommandInteraction,
  queueManager: QueueManager,
  musicPlayer: MusicPlayer,
  _startPlayerUpdate: (guildId: string) => void,
  _cleanupOldMessages: (channelId: string) => Promise<void>
): Promise<void> {
  const guildId = interaction.guildId!;
  const queue = queueManager.getQueue(guildId);

  if (!queue.voiceConnection) {
    await interaction.reply({
      content: "❌ Bot is not in a voice channel",
      ephemeral: true,
    });
    return;
  }

  // Stop music and disconnect
  musicPlayer.stop();
  queue.voiceConnection.destroy();
  queueManager.clearQueue(guildId);

  const embed = new EmbedBuilder()
    .setColor('#FF0000')
    .setTitle('👋 Left Voice Channel')
    .setDescription('Disconnected and cleared queue')
    .setFooter({ text: `Left by ${interaction.user.username}` })
    .setTimestamp();

  await interaction.reply({ embeds: [embed] });
}
