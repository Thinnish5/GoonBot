import {
  ActionRowBuilder,
  ButtonBuilder,
  ButtonStyle,
  Client,
  Collection,
  EmbedBuilder,
  GatewayIntentBits,
  REST,
  Routes,
} from 'discord.js';
import * as dotenv from 'dotenv';
import * as path from 'path';
import * as fs from 'fs';
import { MusicPlayer } from './utils/musicPlayer';
import { QueueManager } from './utils/queueManager';
import { AudioPlayerStatus } from '@discordjs/voice';
import { YouTubeUtil } from './utils/youtubeUtil';

dotenv.config();

const TOKEN = process.env.DISCORD_TOKEN;
const CLIENT_ID = process.env.CLIENT_ID;
const GUILD_ID = process.env.GUILD_ID;

if (!TOKEN || !CLIENT_ID) {
  console.error('Missing DISCORD_TOKEN or CLIENT_ID in .env');
  process.exit(1);
}

// Create Discord client
const client = new Client({
  intents: [
    GatewayIntentBits.Guilds,
    GatewayIntentBits.GuildMembers,
    GatewayIntentBits.GuildVoiceStates,
    GatewayIntentBits.DirectMessages,
  ],
});

// Create instances
const queueManager = new QueueManager();
const musicPlayers = new Map<string, MusicPlayer>();
const playerUpdateIntervals = new Map<string, NodeJS.Timeout>();

const PLAYER_BUTTON_IDS = {
  TOGGLE: 'music:toggle',
  SKIP: 'music:skip',
  STOP: 'music:stop',
  REFRESH: 'music:refresh',
} as const;

function buildProgressBar(elapsed: number, total: number): string {
  const barLength = 20;
  const filledLength = Math.round((elapsed / total) * barLength);
  const emptyLength = barLength - filledLength;
  const bar =
    '█'.repeat(Math.max(0, filledLength - 1)) +
    '🔘' +
    '░'.repeat(Math.max(0, emptyLength - 1));

  const elapsedStr = YouTubeUtil.formatDuration(elapsed);
  const totalStr = YouTubeUtil.formatDuration(total);

  return `\`${elapsedStr}\` ${bar} \`${totalStr}\``;
}

function startPlayerUpdate(guildId: string): void {
  if (playerUpdateIntervals.has(guildId)) {
    clearInterval(playerUpdateIntervals.get(guildId)!);
  }

  const interval = setInterval(async () => {
    const { messageId, channelId } = queueManager.getPlayerMessage(guildId);
    const currentSong = queueManager.getCurrentSong(guildId);

    if (!messageId || !channelId || !currentSong) {
      clearInterval(interval);
      playerUpdateIntervals.delete(guildId);
      return;
    }

    try {
      const channel = await client.channels.fetch(channelId);
      if (!channel || !channel.isTextBased()) return;

      const playerMessage = await channel.messages.fetch(messageId);
      const musicPlayer = getOrCreateMusicPlayer(guildId);

      await playerMessage.edit({
        embeds: [buildPlayerEmbed(guildId)],
        components: buildPlayerControls(guildId, musicPlayer),
      });
    } catch (error) {
      console.error(`Error updating player for guild ${guildId}:`, error);
      clearInterval(interval);
      playerUpdateIntervals.delete(guildId);
    }
  }, 5000);

  playerUpdateIntervals.set(guildId, interval);
}

function stopPlayerUpdate(guildId: string): void {
  if (playerUpdateIntervals.has(guildId)) {
    clearInterval(playerUpdateIntervals.get(guildId)!);
    playerUpdateIntervals.delete(guildId);
  }
}

async function cleanupOldPlayerMessages(channelId: string): Promise<void> {
  try {
    const channel = await client.channels.fetch(channelId);
    if (!channel || !channel.isTextBased()) return;

    // Fetch recent messages (up to last 50 to avoid API limits)
    const messages = await channel.messages.fetch({ limit: 50 });

    // Delete only old player messages (messages with music player buttons)
    for (const message of messages.values()) {
      if (message.author.id === client.user?.id) {
        // Check if this is a player message by looking for music: button IDs
        const isPlayerMessage = (message.components as any[]).some((row: any) =>
          (row.components as any[]).some((component: any) => 
            component.customId && component.customId.startsWith('music:')
          )
        );

        if (isPlayerMessage) {
          try {
            await message.delete();
          } catch (error) {
            console.error('Error deleting player message:', error);
          }
        }
      }
    }
  } catch (error) {
    console.error(`Error cleaning up player messages in channel ${channelId}:`, error);
  }
}

function getOrCreateMusicPlayer(guildId: string): MusicPlayer {
  if (!musicPlayers.has(guildId)) {
    const guildPlayer = new MusicPlayer();

    guildPlayer.getPlayer().on(AudioPlayerStatus.Idle, async () => {
      const queue = queueManager.getQueue(guildId);
      const nextSong = queueManager.dequeueNextSong(guildId);

      if (nextSong && queue.voiceConnection) {
        queueManager.setCurrentSong(guildId, nextSong);
        queueManager.setPlaying(guildId, true);
        queueManager.setSongStartTime(guildId, Date.now());

        try {
          await guildPlayer.playSong(queue.voiceConnection, nextSong);
          startPlayerUpdate(guildId);
        } catch (error) {
          console.error('Error auto-playing next song:', error);
          queueManager.setCurrentSong(guildId, undefined);
          queueManager.setPlaying(guildId, false);
        }

        return;
      }

      queueManager.setCurrentSong(guildId, undefined);
      queueManager.setPlaying(guildId, false);
      stopPlayerUpdate(guildId);
    });

    musicPlayers.set(guildId, guildPlayer);
  }

  return musicPlayers.get(guildId)!;
}

function buildPlayerEmbed(guildId: string): EmbedBuilder {
  const currentSong = queueManager.getCurrentSong(guildId);
  const upcomingSongs = queueManager.getAllSongs(guildId);
  const elapsed = queueManager.getSongElapsedTime(guildId);

  const embed = new EmbedBuilder()
    .setColor('#1DB954')
    .setTitle('🎵 Now Playing')
    .setThumbnail(currentSong?.thumbnail || 'https://via.placeholder.com/160x160?text=No+Song');

  if (currentSong) {
    embed.setDescription(`**${currentSong.title}**`);
    
    // Add progress bar
    const progressBar = buildProgressBar(elapsed, currentSong.duration);
    embed.addFields({
      name: '📊 Progress',
      value: progressBar,
      inline: false,
    });

    embed.addFields(
      {
        name: '⏱️ Duration',
        value: YouTubeUtil.formatDuration(currentSong.duration),
        inline: true,
      },
      {
        name: '📋 Queue Size',
        value: `${upcomingSongs.length} song${upcomingSongs.length !== 1 ? 's' : ''}`,
        inline: true,
      }
    );
  } else {
    embed.setDescription('No song is currently playing.');
  }

  if (upcomingSongs.length > 0) {
    const upcomingPreview = upcomingSongs
      .slice(0, 5)
      .map((song, index) => `${index + 1}. [${song.title}](${song.url})`)
      .join('\n');

    const footerText =
      upcomingSongs.length > 5 ? `... and ${upcomingSongs.length - 5} more songs` : 'End of queue';

    embed.addFields({
      name: '📋 Up Next',
      value: upcomingPreview || 'No upcoming songs',
      inline: false,
    });

    embed.setFooter({ text: footerText });
  } else {
    embed.setFooter({ text: 'Queue will end after current song' });
  }

  return embed;
}

function buildPlayerControls(guildId: string, musicPlayer: MusicPlayer) {
  const hasCurrentSong = Boolean(queueManager.getCurrentSong(guildId));
  const isPlayingNow = musicPlayer.isPlaying();

  const controlRow = new ActionRowBuilder<ButtonBuilder>().addComponents(
    new ButtonBuilder()
      .setCustomId(PLAYER_BUTTON_IDS.TOGGLE)
      .setEmoji(isPlayingNow ? '⏸️' : '▶️')
      .setStyle(ButtonStyle.Success)
      .setDisabled(!hasCurrentSong),
    new ButtonBuilder()
      .setCustomId(PLAYER_BUTTON_IDS.SKIP)
      .setEmoji('⏭️')
      .setStyle(ButtonStyle.Secondary)
      .setDisabled(!hasCurrentSong),
    new ButtonBuilder()
      .setCustomId(PLAYER_BUTTON_IDS.STOP)
      .setEmoji('⏹️')
      .setStyle(ButtonStyle.Danger)
      .setDisabled(!hasCurrentSong)
  );

  const infoRow = new ActionRowBuilder<ButtonBuilder>().addComponents(
    new ButtonBuilder()
      .setCustomId(PLAYER_BUTTON_IDS.REFRESH)
      .setLabel('Queue')
      .setEmoji('📋')
      .setStyle(ButtonStyle.Secondary)
  );

  return [controlRow, infoRow];
}

// Load commands
interface Command {
  data: any;
  execute: (interaction: any, queueManager: QueueManager, musicPlayer: MusicPlayer, startPlayerUpdate: (guildId: string) => void, cleanupOldMessages: (channelId: string) => Promise<void>) => Promise<void>;
}

const commands = new Collection<string, Command>();
const commandsPath = path.join(__dirname, 'commands');
const commandFiles = fs.readdirSync(commandsPath).filter((file) => file.endsWith('.ts') || file.endsWith('.js'));

for (const file of commandFiles) {
  const filePath = path.join(commandsPath, file);
  const command = require(filePath) as Command;
  if ('data' in command && 'execute' in command) {
    commands.set(command.data.name, command);
    console.log(`✅ Loaded command: ${command.data.name}`);
  }
}

// Bot is ready
client.once('clientReady', async () => {
  console.log(`✅ Logged in as ${client.user?.tag}`);

  // Register slash commands
  const commandsData = commands.map((cmd) => cmd.data.toJSON());
  const rest = new REST({ version: '10' }).setToken(TOKEN!);

  try {
    if (GUILD_ID) {
      // Register to specific guild (faster for testing)
      await rest.put(Routes.applicationGuildCommands(CLIENT_ID!, GUILD_ID), {
        body: commandsData,
      });
      console.log('✅ Registered commands to guild');
    } else {
      // Register globally (takes up to 1 hour)
      await rest.put(Routes.applicationCommands(CLIENT_ID!), {
        body: commandsData,
      });
      console.log('✅ Registered commands globally');
    }
  } catch (error) {
    console.error('Error registering commands:', error);
  }
});

// Handle slash commands
client.on('interactionCreate', async (interaction) => {
  const guildId = interaction.guildId;
  if (!guildId) {
    return;
  }

  const musicPlayer = getOrCreateMusicPlayer(guildId);

  if (interaction.isButton() && interaction.customId.startsWith('music:')) {
    try {
      await interaction.deferUpdate();
      const currentSong = queueManager.getCurrentSong(guildId);
      const { messageId, channelId } = queueManager.getPlayerMessage(guildId);

      if (!messageId || !channelId) {
        await interaction.followUp({ content: 'Player message not found.', ephemeral: true });
        return;
      }

      const channel = await client.channels.fetch(channelId);
      if (!channel || !channel.isTextBased()) {
        return;
      }

      const playerMessage = await channel.messages.fetch(messageId);

      if (interaction.customId === PLAYER_BUTTON_IDS.REFRESH) {
        await playerMessage.edit({
          embeds: [buildPlayerEmbed(guildId)],
          components: buildPlayerControls(guildId, musicPlayer),
        });
        return;
      }

      if (!currentSong) {
        await interaction.followUp({ content: 'Nothing is currently playing.', ephemeral: true });
        return;
      }

      if (interaction.customId === PLAYER_BUTTON_IDS.TOGGLE) {
        if (musicPlayer.isPlaying()) {
          musicPlayer.pause();
        } else {
          musicPlayer.unpause();
        }

        await playerMessage.edit({
          embeds: [buildPlayerEmbed(guildId)],
          components: buildPlayerControls(guildId, musicPlayer),
        });
        return;
      }

      if (interaction.customId === PLAYER_BUTTON_IDS.SKIP) {
        const nextSong = queueManager.dequeueNextSong(guildId);
        queueManager.setCurrentSong(guildId, undefined);

        if (nextSong) {
          const queue = queueManager.getQueue(guildId);
          if (queue.voiceConnection) {
            queueManager.setCurrentSong(guildId, nextSong);
            queueManager.setPlaying(guildId, true);
            queueManager.setSongStartTime(guildId, Date.now());
            await musicPlayer.playSong(queue.voiceConnection, nextSong);
            startPlayerUpdate(guildId);
          }
        } else {
          musicPlayer.stop();
          queueManager.setPlaying(guildId, false);
          stopPlayerUpdate(guildId);
        }

        await playerMessage.edit({
          embeds: [buildPlayerEmbed(guildId)],
          components: buildPlayerControls(guildId, musicPlayer),
        });
        return;
      }

      if (interaction.customId === PLAYER_BUTTON_IDS.STOP) {
        const queue = queueManager.getQueue(guildId);
        musicPlayer.stop();
        queue.voiceConnection?.destroy();
        queueManager.clearQueue(guildId);
        stopPlayerUpdate(guildId);

        await playerMessage.edit({
          embeds: [buildPlayerEmbed(guildId)],
          components: buildPlayerControls(guildId, musicPlayer),
        });
      }
    } catch (error) {
      console.error('Error handling music button:', error);
      if (!interaction.replied && !interaction.deferred) {
        await interaction.reply({ content: 'Failed to handle player action.', ephemeral: true });
      }
    }
    return;
  }

  if (!interaction.isChatInputCommand()) return;

  const command = commands.get(interaction.commandName);
  if (!command) {
    console.error(`No command matching ${interaction.commandName} was found.`);
    return;
  }

  try {
    await command.execute(interaction, queueManager, musicPlayer, startPlayerUpdate, cleanupOldPlayerMessages);
  } catch (error) {
    console.error(`Error executing ${interaction.commandName}:`, error);
    if (!interaction.replied) {
      await interaction.reply({
        content: '❌ There was an error while executing this command!',
        ephemeral: true,
      });
    }
  }
});

// Handle voice state changes
client.on('voiceStateUpdate', (oldState, newState) => {
  // Leave voice channel if bot is alone
  if (newState.guild.members.me?.voice.channel) {
    const channel = newState.guild.members.me.voice.channel;
    const members = channel.members.filter((m) => !m.user.bot);

    if (members.size === 0) {
      const connection = newState.guild.voiceAdapterCreator as any;
      if (connection) {
        connection.disconnect();
        queueManager.clearQueue(newState.guild.id);
      }
    }
  }
});

// Handle player end event for auto-skip
client.on('raw', async (event) => {
  if (event.t !== 'VOICE_SERVER_UPDATE' && event.t !== 'VOICE_STATE_UPDATE') return;
});

// Login to Discord
client.login(TOKEN);

// Graceful shutdown
process.on('SIGINT', () => {
  console.log('\n👋 Shutting down gracefully...');
  client.destroy();
  process.exit(0);
});
