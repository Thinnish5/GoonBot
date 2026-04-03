import {
  ActionRowBuilder,
  ButtonBuilder,
  ButtonStyle,
  ChatInputCommandInteraction,
  EmbedBuilder,
  SlashCommandBuilder,
} from 'discord.js';
import { VoiceConnectionStatus, entersState, joinVoiceChannel } from '@discordjs/voice';
import { YouTubeUtil } from '../utils/youtubeUtil';
import { MusicPlayer } from '../utils/musicPlayer';
import { QueueManager } from '../utils/queueManager';

export const data = new SlashCommandBuilder()
  .setName('play')
  .setDescription('Play a song from YouTube')
  .addStringOption((option) =>
    option
      .setName('url')
      .setDescription('YouTube URL or song name')
      .setRequired(true)
  );

export async function execute(
  interaction: ChatInputCommandInteraction,
  queueManager: QueueManager,
  musicPlayer: MusicPlayer,
  startPlayerUpdate: (guildId: string) => void,
  cleanupOldMessages: (channelId: string) => Promise<void>
): Promise<void> {
  const urlOrQuery = interaction.options.getString('url', true);
  const member = interaction.member as any;

  if (!member?.voice?.channel) {
    await interaction.reply("❌ You must be in a voice channel to use this command!");
    return;
  }

  // Cleanup old messages BEFORE deferReply to avoid deleting the deferred message
  await cleanupOldMessages(interaction.channelId!);

  await interaction.deferReply();

  try {
    // Validate and fetch song info
    if (!(await YouTubeUtil.isValidUrl(urlOrQuery))) {
      await interaction.editReply("❌ Invalid YouTube URL!");
      return;
    }

    const songInfo = await YouTubeUtil.fetchSongInfo(urlOrQuery, interaction.user.id);
    const queue = queueManager.getQueue(interaction.guildId!);

    // Add song to queue
    queueManager.addSong(interaction.guildId!, songInfo);

    // Create embed response
    const queueSize = queueManager.getQueueSize(interaction.guildId!);
    const isFirstOrSoloSong = queueSize === 1;

    const embed = new EmbedBuilder()
      .setColor('#1DB954')
      .setTitle(isFirstOrSoloSong ? '▶️ Now Playing' : '✅ Added to Queue')
      .setDescription(`**${songInfo.title}**`)
      .setThumbnail(songInfo.thumbnail || 'https://via.placeholder.com/160x160?text=YouTube')
      .addFields(
        {
          name: '⏱️ Duration',
          value: YouTubeUtil.formatDuration(songInfo.duration),
          inline: true,
        },
        {
          name: isFirstOrSoloSong ? '🎯 Status' : '📍 Position',
          value: isFirstOrSoloSong ? 'Now Playing' : `#${queueSize} in Queue`,
          inline: true,
        }
      )
      .setFooter({ text: `Added by ${interaction.user.username}` })
      .setTimestamp();

    const controlRow = new ActionRowBuilder<ButtonBuilder>().addComponents(
      new ButtonBuilder()
        .setCustomId('music:toggle')
        .setEmoji(musicPlayer.isPlaying() ? '⏸️' : '▶️')
        .setStyle(ButtonStyle.Success),
      new ButtonBuilder()
        .setCustomId('music:skip')
        .setEmoji('⏭️')
        .setStyle(ButtonStyle.Secondary),
      new ButtonBuilder()
        .setCustomId('music:stop')
        .setEmoji('⏹️')
        .setStyle(ButtonStyle.Danger)
    );

    const infoRow = new ActionRowBuilder<ButtonBuilder>().addComponents(
      new ButtonBuilder()
        .setCustomId('music:refresh')
        .setLabel('Queue')
        .setEmoji('📋')
        .setStyle(ButtonStyle.Secondary)
    );

    const playerMsg = await interaction.editReply({ embeds: [embed], components: [controlRow, infoRow] });
    queueManager.setPlayerMessage(interaction.guildId!, playerMsg.id, interaction.channelId!);
    startPlayerUpdate(interaction.guildId!);

    // If not currently playing, start playback
    if (!queueManager.isPlaying(interaction.guildId!)) {
      await startPlayback(interaction, queueManager, musicPlayer);
    }
  } catch (error) {
    console.error('Play command error:', error);
    await interaction.editReply("❌ Error processing your request. Please make sure the URL is valid.");
  }
}

async function startPlayback(
  interaction: ChatInputCommandInteraction,
  queueManager: QueueManager,
  musicPlayer: MusicPlayer
): Promise<void> {
  const member = interaction.member as any;
  const channel = member.voice.channel;

  if (!channel) {
    return;
  }

  try {
    const queue = queueManager.getQueue(interaction.guildId!);
    let connection = queue.voiceConnection;

    if (!connection || connection.state.status === VoiceConnectionStatus.Destroyed) {
      connection = joinVoiceChannel({
        channelId: channel.id,
        guildId: interaction.guildId!,
        adapterCreator: (interaction.guild as any).voiceAdapterCreator,
      });
      await entersState(connection, VoiceConnectionStatus.Ready, 20_000);
      queue.voiceConnection = connection;
    }

    const nextSong = queueManager.dequeueNextSong(interaction.guildId!);
    if (nextSong) {
      queueManager.setCurrentSong(interaction.guildId!, nextSong);
      queueManager.setPlaying(interaction.guildId!, true);
      queueManager.setSongStartTime(interaction.guildId!, Date.now());
      await musicPlayer.playSong(connection, nextSong);
    }
  } catch (error) {
    console.error('Error starting playback:', error);
    queueManager.setCurrentSong(interaction.guildId!, undefined);
    queueManager.setPlaying(interaction.guildId!, false);
  }
}
