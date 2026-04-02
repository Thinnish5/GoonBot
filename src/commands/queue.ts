import {
  ActionRowBuilder,
  ButtonBuilder,
  ButtonStyle,
  SlashCommandBuilder,
  ChatInputCommandInteraction,
  EmbedBuilder,
} from 'discord.js';
import { QueueManager } from '../utils/queueManager';
import { YouTubeUtil } from '../utils/youtubeUtil';
import { MusicPlayer } from '../utils/musicPlayer';

export const data = new SlashCommandBuilder()
  .setName('queue')
  .setDescription('Show the current music queue');

export async function execute(
  interaction: ChatInputCommandInteraction,
  queueManager: QueueManager,
  musicPlayer: MusicPlayer,
  startPlayerUpdate: (guildId: string) => void,
  cleanupOldMessages: (channelId: string) => Promise<void>
): Promise<void> {
  const guildId = interaction.guildId!;
  const currentSong = queueManager.getCurrentSong(guildId);
  const upcomingSongs = queueManager.getAllSongs(guildId);

  if (!currentSong && upcomingSongs.length === 0) {
    await interaction.reply({ content: '🎵 Queue is empty', ephemeral: true });
    return;
  }

  // Cleanup old messages BEFORE replying
  await cleanupOldMessages(interaction.channelId!);

  const embed = new EmbedBuilder()
    .setColor('#1DB954')
    .setTitle('🎵 Music Queue')
    .setThumbnail(currentSong?.thumbnail || 'https://via.placeholder.com/160x160?text=No+Song');

  // Current song
  if (currentSong) {
    embed.setDescription(`**${currentSong.title}**\n\n\`${YouTubeUtil.formatDuration(currentSong.duration)}\``);
  }

  // Upcoming songs
  if (upcomingSongs.length > 0) {
    const upcomingList = upcomingSongs
      .slice(0, 7)
      .map(
        (song, index) =>
          `${index + 1}. [${song.title}](${song.url})`
      )
      .join('\n');

    embed.addFields({
      name: `📋 Up Next (${upcomingSongs.length})`,
      value: upcomingList || 'No upcoming songs',
      inline: false,
    });

    if (upcomingSongs.length > 7) {
      embed.setFooter({ text: `... and ${upcomingSongs.length - 7} more songs` });
    }
  }

  const controlRow = new ActionRowBuilder<ButtonBuilder>().addComponents(
    new ButtonBuilder()
      .setCustomId('music:toggle')
      .setEmoji('▶️')
      .setStyle(ButtonStyle.Success)
      .setDisabled(!currentSong),
    new ButtonBuilder()
      .setCustomId('music:skip')
      .setEmoji('⏭️')
      .setStyle(ButtonStyle.Secondary)
      .setDisabled(!currentSong),
    new ButtonBuilder()
      .setCustomId('music:stop')
      .setEmoji('⏹️')
      .setStyle(ButtonStyle.Danger)
      .setDisabled(!currentSong)
  );

  const infoRow = new ActionRowBuilder<ButtonBuilder>().addComponents(
    new ButtonBuilder()
      .setCustomId('music:refresh')
      .setLabel('Refresh')
      .setEmoji('🔄')
      .setStyle(ButtonStyle.Secondary)
  );

  const playerMsg = await interaction.reply({ embeds: [embed], components: [controlRow, infoRow] });
  queueManager.setPlayerMessage(interaction.guildId!, playerMsg.id, interaction.channelId!);
  startPlayerUpdate(interaction.guildId!);
}
