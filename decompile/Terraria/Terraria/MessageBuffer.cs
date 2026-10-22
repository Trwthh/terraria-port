using System;
using System.Collections.Generic;
using System.IO;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics.PackedVector;
using Terraria.Audio;
using Terraria.Chat;
using Terraria.DataStructures;
using Terraria.Enums;
using Terraria.GameContent;
using Terraria.GameContent.Achievements;
using Terraria.GameContent.Creative;
using Terraria.GameContent.Events;
using Terraria.GameContent.Golf;
using Terraria.GameContent.Tile_Entities;
using Terraria.GameContent.UI;
using Terraria.ID;
using Terraria.Localization;
using Terraria.Map;
using Terraria.Net;
using Terraria.Net.Sockets;
using Terraria.Social;
using Terraria.Social.Steam;
using Terraria.Testing;
using Terraria.UI;

namespace Terraria
{
	public class MessageBuffer
	{
		public const int readBufferMax = 131070;

		public const int writeBufferMax = 131070;

		public bool broadcast;

		public byte[] readBuffer = new byte[131070];

		public byte[] writeBuffer = new byte[131070];

		public bool writeLocked;

		public int messageLength;

		public int totalData;

		public int whoAmI;

		public int spamCount;

		public int maxSpam;

		public bool checkBytes;

		public MemoryStream readerStream;

		public MemoryStream writerStream;

		public BinaryReader reader;

		public BinaryWriter writer;

		public PacketHistory History = new PacketHistory();

		private float[] _temporaryProjectileAI = new float[Projectile.maxAI];

		private float[] _temporaryNPCAI = new float[NPC.maxAI];

		public int RemainingReadBufferLength => readBuffer.Length - totalData;

		public static event TileChangeReceivedEvent OnTileChangeReceived;

		public void Reset()
		{
			Array.Clear(readBuffer, 0, readBuffer.Length);
			Array.Clear(writeBuffer, 0, writeBuffer.Length);
			writeLocked = false;
			messageLength = 0;
			totalData = 0;
			spamCount = 0;
			broadcast = false;
			checkBytes = false;
			ResetReader();
			ResetWriter();
		}

		public void ResetReader()
		{
			if (readerStream != null)
			{
				readerStream.Close();
			}
			readerStream = new MemoryStream(readBuffer);
			reader = new BinaryReader(readerStream);
		}

		public void ResetWriter()
		{
			if (writerStream != null)
			{
				writerStream.Close();
			}
			writerStream = new MemoryStream(writeBuffer);
			writer = new BinaryWriter(writerStream);
		}

		private float[] ReUseTemporaryProjectileAI()
		{
			for (int i = 0; i < _temporaryProjectileAI.Length; i++)
			{
				_temporaryProjectileAI[i] = 0f;
			}
			return _temporaryProjectileAI;
		}

		private float[] ReUseTemporaryNPCAI()
		{
			for (int i = 0; i < _temporaryNPCAI.Length; i++)
			{
				_temporaryNPCAI[i] = 0f;
			}
			return _temporaryNPCAI;
		}

		public void GetData(int start, int length, out int messageType)
		{
			if (whoAmI < 256)
			{
				Netplay.Clients[whoAmI].TimeOutTimer = 0;
			}
			else
			{
				Netplay.Connection.TimeOutTimer = 0;
			}
			byte b = 0;
			int num = 0;
			num = start + 1;
			b = (byte)(messageType = readBuffer[start]);
			if (b >= MessageID.Count)
			{
				return;
			}
			Main.ActiveNetDiagnosticsUI.CountReadMessage(b, length);
			if (Main.netMode == 1 && Netplay.Connection.StatusMax > 0)
			{
				Netplay.Connection.StatusCount++;
			}
			if (Main.verboseNetplay)
			{
				for (int i = start; i < start + length; i++)
				{
				}
				for (int j = start; j < start + length; j++)
				{
					_ = readBuffer[j];
				}
			}
			if (Main.netMode == 2 && b != 38 && Netplay.Clients[whoAmI].State == -1)
			{
				NetMessage.TrySendData(2, whoAmI, -1, Lang.mp[1].ToNetworkText());
				return;
			}
			if (Main.netMode == 2)
			{
				if (Netplay.Clients[whoAmI].State < 10 && b > 12 && b != 93 && b != 16 && b != 42 && b != 50 && b != 38 && b != 68 && b != 147 && b != 161)
				{
					NetMessage.BootPlayer(whoAmI, Lang.mp[2].ToNetworkText());
				}
				if (Netplay.Clients[whoAmI].State == 0 && b != 1)
				{
					NetMessage.BootPlayer(whoAmI, Lang.mp[2].ToNetworkText());
				}
			}
			if (reader == null)
			{
				ResetReader();
			}
			reader.BaseStream.Position = num;
			switch (b)
			{
			case 1:
			{
				if (Main.netMode != 2)
				{
					break;
				}
				NetServerSocialModule netServerSocialModule = SocialAPI.Network as NetServerSocialModule;
				RemoteAddress remoteAddress = Netplay.Clients[whoAmI].Socket.GetRemoteAddress();
				if (netServerSocialModule != null && netServerSocialModule.ShouldBlockAsNotFriends(remoteAddress))
				{
					NetMessage.TrySendData(2, whoAmI, -1, NetworkText.FromKey("Net.SteamHostOnlyAllowsFriends"));
				}
				else if (Main.dedServ && Netplay.IsBanned(remoteAddress))
				{
					NetMessage.TrySendData(2, whoAmI, -1, Lang.mp[3].ToNetworkText());
				}
				else
				{
					if (Netplay.Clients[whoAmI].State != 0)
					{
						break;
					}
					if (reader.ReadString() == "Terraria" + 326)
					{
						if (string.IsNullOrEmpty(Netplay.ServerPassword))
						{
							Netplay.Clients[whoAmI].State = 1;
							NetMessage.TrySendData(3, whoAmI);
						}
						else
						{
							Netplay.Clients[whoAmI].State = -1;
							NetMessage.TrySendData(37, whoAmI);
						}
					}
					else
					{
						NetMessage.TrySendData(2, whoAmI, -1, Lang.mp[4].ToNetworkText());
					}
				}
				break;
			}
			case 2:
				if (Main.netMode == 1)
				{
					Netplay.Disconnect = true;
					Main.statusText = NetworkText.Deserialize(reader).ToString();
				}
				break;
			case 3:
				if (Main.netMode == 1)
				{
					if (Netplay.Connection.State == 1)
					{
						Netplay.Connection.State = 2;
					}
					int num134 = reader.ReadByte();
					bool value3 = reader.ReadBoolean();
					Netplay.Connection.ServerSpecialFlags[2] = value3;
					if (num134 != Main.myPlayer)
					{
						Main.player[num134] = Main.ActivePlayerFileData.Player;
						Main.player[Main.myPlayer] = new Player();
					}
					Main.player[num134].whoAmI = num134;
					Main.myPlayer = num134;
					Player player17 = Main.player[num134];
					NetMessage.TrySendData(4, -1, -1, null, num134);
					NetMessage.TrySendData(68, -1, -1, null, num134);
					NetMessage.TrySendData(16, -1, -1, null, num134);
					NetMessage.TrySendData(42, -1, -1, null, num134);
					NetMessage.TrySendData(50, -1, -1, null, num134);
					NetMessage.TrySendData(147, -1, -1, null, num134, player17.CurrentLoadoutIndex);
					for (int num135 = 0; num135 < 59; num135++)
					{
						NetMessage.TrySendData(5, -1, -1, null, num134, PlayerItemSlotID.Inventory0 + num135);
					}
					TrySendingItemArray(num134, player17.armor, PlayerItemSlotID.Armor0);
					TrySendingItemArray(num134, player17.dye, PlayerItemSlotID.Dye0);
					TrySendingItemArray(num134, player17.miscEquips, PlayerItemSlotID.Misc0);
					TrySendingItemArray(num134, player17.miscDyes, PlayerItemSlotID.MiscDye0);
					TrySendingItemArray(num134, player17.bank.item, PlayerItemSlotID.Bank1_0);
					TrySendingItemArray(num134, player17.bank2.item, PlayerItemSlotID.Bank2_0);
					NetMessage.TrySendData(5, -1, -1, null, num134, PlayerItemSlotID.TrashItem);
					TrySendingItemArray(num134, player17.bank3.item, PlayerItemSlotID.Bank3_0);
					TrySendingItemArray(num134, player17.bank4.item, PlayerItemSlotID.Bank4_0);
					TrySendingItemArray(num134, player17.Loadouts[0].Armor, PlayerItemSlotID.Loadout1_Armor_0);
					TrySendingItemArray(num134, player17.Loadouts[0].Dye, PlayerItemSlotID.Loadout1_Dye_0);
					TrySendingItemArray(num134, player17.Loadouts[1].Armor, PlayerItemSlotID.Loadout2_Armor_0);
					TrySendingItemArray(num134, player17.Loadouts[1].Dye, PlayerItemSlotID.Loadout2_Dye_0);
					TrySendingItemArray(num134, player17.Loadouts[2].Armor, PlayerItemSlotID.Loadout3_Armor_0);
					TrySendingItemArray(num134, player17.Loadouts[2].Dye, PlayerItemSlotID.Loadout3_Dye_0);
					if (!string.IsNullOrWhiteSpace(Netplay.HostToken))
					{
						NetMessage.TrySendData(161, -1, -1, NetworkText.FromLiteral(Netplay.HostToken));
					}
					NetMessage.TrySendData(6);
					if (Netplay.Connection.State == 2)
					{
						Netplay.Connection.State = 3;
					}
				}
				break;
			case 4:
			{
				int num188 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num188 = whoAmI;
				}
				if (num188 == Main.myPlayer && !Main.ServerSideCharacter)
				{
					break;
				}
				Player player18 = Main.player[num188];
				player18.whoAmI = num188;
				player18.skinVariant = reader.ReadByte();
				player18.skinVariant = (int)MathHelper.Clamp(player18.skinVariant, 0f, PlayerVariantID.Count - 1);
				player18.voiceVariant = reader.ReadByte();
				player18.voiceVariant = Utils.Clamp(player18.voiceVariant, 1, 4);
				player18.voicePitchOffset = reader.ReadSingle();
				if (float.IsNaN(player18.voicePitchOffset))
				{
					player18.voicePitchOffset = 0f;
				}
				player18.voicePitchOffset = Utils.Clamp(player18.voicePitchOffset, -1f, 1f);
				player18.hair = reader.ReadByte();
				if (player18.hair >= 228)
				{
					player18.hair = 0;
				}
				player18.name = reader.ReadString().Trim().Trim();
				player18.hairDye = reader.ReadByte();
				ReadAccessoryVisibility(reader, player18.hideVisibleAccessory);
				player18.hideMisc = reader.ReadByte();
				player18.hairColor = reader.ReadRGB();
				player18.skinColor = reader.ReadRGB();
				player18.eyeColor = reader.ReadRGB();
				player18.shirtColor = reader.ReadRGB();
				player18.underShirtColor = reader.ReadRGB();
				player18.pantsColor = reader.ReadRGB();
				player18.shoeColor = reader.ReadRGB();
				BitsByte bitsByte32 = reader.ReadByte();
				player18.difficulty = 0;
				if (bitsByte32[0])
				{
					player18.difficulty = 1;
				}
				if (bitsByte32[1])
				{
					player18.difficulty = 2;
				}
				if (bitsByte32[3])
				{
					player18.difficulty = 3;
				}
				if (player18.difficulty > 3)
				{
					player18.difficulty = 3;
				}
				player18.extraAccessory = bitsByte32[2];
				BitsByte bitsByte33 = reader.ReadByte();
				player18.UsingBiomeTorches = bitsByte33[0];
				player18.happyFunTorchTime = bitsByte33[1];
				player18.unlockedBiomeTorches = bitsByte33[2];
				player18.unlockedSuperCart = bitsByte33[3];
				player18.enabledSuperCart = bitsByte33[4];
				BitsByte bitsByte34 = reader.ReadByte();
				player18.usedAegisCrystal = bitsByte34[0];
				player18.usedAegisFruit = bitsByte34[1];
				player18.usedArcaneCrystal = bitsByte34[2];
				player18.usedGalaxyPearl = bitsByte34[3];
				player18.usedGummyWorm = bitsByte34[4];
				player18.usedAmbrosia = bitsByte34[5];
				player18.ateArtisanBread = bitsByte34[6];
				if (Main.netMode != 2)
				{
					break;
				}
				bool flag19 = false;
				if (Netplay.Clients[whoAmI].State < 10)
				{
					for (int num189 = 0; num189 < 255; num189++)
					{
						if (num189 != num188 && player18.name == Main.player[num189].name && Netplay.Clients[num189].IsActive)
						{
							flag19 = true;
						}
					}
				}
				if (flag19)
				{
					NetMessage.TrySendData(2, whoAmI, -1, NetworkText.FromKey(Lang.mp[5].Key, player18.name));
				}
				else if (player18.name.Length > Player.nameLen)
				{
					NetMessage.TrySendData(2, whoAmI, -1, NetworkText.FromKey("Net.NameTooLong"));
				}
				else if (player18.name == "")
				{
					NetMessage.TrySendData(2, whoAmI, -1, NetworkText.FromKey("Net.EmptyName"));
				}
				else if (player18.difficulty == 3 && !Main.IsJourneyMode)
				{
					NetMessage.TrySendData(2, whoAmI, -1, NetworkText.FromKey("Net.PlayerIsCreativeAndWorldIsNotCreative"));
				}
				else if (player18.difficulty != 3 && Main.IsJourneyMode)
				{
					NetMessage.TrySendData(2, whoAmI, -1, NetworkText.FromKey("Net.PlayerIsNotCreativeAndWorldIsCreative"));
				}
				else
				{
					Netplay.Clients[whoAmI].Name = player18.name;
					Netplay.Clients[whoAmI].Name = player18.name;
					NetMessage.TrySendData(4, -1, whoAmI, null, num188);
				}
				break;
			}
			case 5:
			{
				int num25 = reader.ReadByte();
				int num26 = reader.ReadInt16();
				int stack2 = reader.ReadInt16();
				int prefixWeWant = reader.ReadByte();
				int type2 = reader.ReadInt16();
				BitsByte bitsByte2 = reader.ReadByte();
				bool favorited = bitsByte2[0];
				bool flag = bitsByte2[1];
				if (Main.netMode == 2)
				{
					num25 = whoAmI;
				}
				if (num25 == Main.myPlayer && !Main.ServerSideCharacter && !Main.player[num25].HasLockedInventory())
				{
					break;
				}
				Player player2 = Main.player[num25];
				lock (player2)
				{
					PlayerItemSlotID.SlotReference slot = new PlayerItemSlotID.SlotReference(player2, num26);
					PlayerItemSlotID.SlotReference slotReference = new PlayerItemSlotID.SlotReference(Main.clientPlayer, num26);
					Item item = new Item();
					item.SetDefaults(type2);
					item.stack = stack2;
					item.Prefix(prefixWeWant);
					item.favorited = favorited;
					slot.Item = item;
					if (num25 == Main.myPlayer && !Main.ServerSideCharacter)
					{
						slotReference.Item = item.Clone();
					}
					if (num26 >= PlayerItemSlotID.Bank4_0 && num26 < PlayerItemSlotID.Loadout1_Armor_0)
					{
						if (Main.netMode == 1 && player2.disableVoidBag == num26 - PlayerItemSlotID.Bank4_0)
						{
							player2.disableVoidBag = -1;
						}
					}
					else if (num26 <= 58)
					{
						if (num25 == Main.myPlayer && num26 == 58)
						{
							Main.mouseItem = item.Clone();
						}
						if (num25 == Main.myPlayer && Main.netMode == 1)
						{
							Main.player[num25].inventoryChestStack[num26] = false;
						}
					}
					if (Main.netMode == 1 && num25 == Main.myPlayer && flag)
					{
						ItemSlot.IndicateBlockedSlot(slot);
					}
					bool[] canRelay = PlayerItemSlotID.CanRelay;
					if (Main.netMode == 2 && num25 == whoAmI && canRelay.IndexInRange(num26) && canRelay[num26])
					{
						NetMessage.TrySendData(5, -1, whoAmI, null, num25, num26);
					}
					break;
				}
			}
			case 6:
				if (Main.netMode == 2)
				{
					if (Netplay.Clients[whoAmI].State == 1)
					{
						Netplay.Clients[whoAmI].State = 2;
					}
					NetMessage.TrySendData(7, whoAmI);
					Main.SyncAnInvasion(whoAmI);
				}
				break;
			case 7:
				if (Main.netMode == 1)
				{
					Main.time = reader.ReadInt32();
					BitsByte bitsByte6 = reader.ReadByte();
					Main.dayTime = bitsByte6[0];
					Main.bloodMoon = bitsByte6[1];
					Main.eclipse = bitsByte6[2];
					Main.moonPhase = reader.ReadByte();
					Main.maxTilesX = reader.ReadInt16();
					Main.maxTilesY = reader.ReadInt16();
					Main.spawnTileX = reader.ReadInt16();
					Main.spawnTileY = reader.ReadInt16();
					Main.worldSurface = reader.ReadInt16();
					Main.rockLayer = reader.ReadInt16();
					Main.ActiveWorldFileData.WorldId = reader.ReadInt32();
					Main.worldName = reader.ReadString();
					Main.GameMode = reader.ReadByte();
					Main.ActiveWorldFileData.UniqueId = new Guid(reader.ReadBytes(16));
					Main.ActiveWorldFileData.WorldGeneratorVersion = reader.ReadUInt64();
					Main.moonType = reader.ReadByte();
					WorldGen.setBG(0, reader.ReadByte());
					WorldGen.setBG(10, reader.ReadByte());
					WorldGen.setBG(11, reader.ReadByte());
					WorldGen.setBG(12, reader.ReadByte());
					WorldGen.setBG(1, reader.ReadByte());
					WorldGen.setBG(2, reader.ReadByte());
					WorldGen.setBG(3, reader.ReadByte());
					WorldGen.setBG(4, reader.ReadByte());
					WorldGen.setBG(5, reader.ReadByte());
					WorldGen.setBG(6, reader.ReadByte());
					WorldGen.setBG(7, reader.ReadByte());
					WorldGen.setBG(8, reader.ReadByte());
					WorldGen.setBG(9, reader.ReadByte());
					Main.iceBackStyle = reader.ReadByte();
					Main.jungleBackStyle = reader.ReadByte();
					Main.hellBackStyle = reader.ReadByte();
					Main.windSpeedTarget = reader.ReadSingle();
					Main.numClouds = reader.ReadByte();
					for (int l = 0; l < 3; l++)
					{
						Main.treeX[l] = reader.ReadInt32();
					}
					for (int m = 0; m < 4; m++)
					{
						Main.treeStyle[m] = reader.ReadByte();
					}
					for (int n = 0; n < 3; n++)
					{
						Main.caveBackX[n] = reader.ReadInt32();
					}
					for (int num75 = 0; num75 < 4; num75++)
					{
						Main.caveBackStyle[num75] = reader.ReadByte();
					}
					WorldGen.TreeTops.SyncReceive(reader);
					WorldGen.BackgroundsCache.UpdateCache();
					Main.maxRaining = reader.ReadSingle();
					Main.raining = Main.maxRaining > 0f;
					BitsByte bitsByte7 = reader.ReadByte();
					WorldGen.shadowOrbSmashed = bitsByte7[0];
					NPC.downedBoss1 = bitsByte7[1];
					NPC.downedBoss2 = bitsByte7[2];
					NPC.downedBoss3 = bitsByte7[3];
					Main.hardMode = bitsByte7[4];
					NPC.downedClown = bitsByte7[5];
					Main.ServerSideCharacter = bitsByte7[6];
					NPC.downedPlantBoss = bitsByte7[7];
					if (Main.ServerSideCharacter)
					{
						Main.ActivePlayerFileData.MarkAsServerSide();
					}
					BitsByte bitsByte8 = reader.ReadByte();
					NPC.downedMechBoss1 = bitsByte8[0];
					NPC.downedMechBoss2 = bitsByte8[1];
					NPC.downedMechBoss3 = bitsByte8[2];
					NPC.downedMechBossAny = bitsByte8[3];
					Main.cloudBGActive = (bitsByte8[4] ? 1 : 0);
					WorldGen.crimson = bitsByte8[5];
					Main.pumpkinMoon = bitsByte8[6];
					Main.snowMoon = bitsByte8[7];
					BitsByte bitsByte9 = reader.ReadByte();
					Main.fastForwardTimeToDawn = bitsByte9[1];
					Main.UpdateTimeRate();
					bool num76 = bitsByte9[2];
					NPC.downedSlimeKing = bitsByte9[3];
					NPC.downedQueenBee = bitsByte9[4];
					NPC.downedFishron = bitsByte9[5];
					NPC.downedMartians = bitsByte9[6];
					NPC.downedAncientCultist = bitsByte9[7];
					BitsByte bitsByte10 = reader.ReadByte();
					NPC.downedMoonlord = bitsByte10[0];
					NPC.downedHalloweenKing = bitsByte10[1];
					NPC.downedHalloweenTree = bitsByte10[2];
					NPC.downedChristmasIceQueen = bitsByte10[3];
					NPC.downedChristmasSantank = bitsByte10[4];
					NPC.downedChristmasTree = bitsByte10[5];
					NPC.downedGolemBoss = bitsByte10[6];
					BirthdayParty.ManualParty = bitsByte10[7];
					BitsByte bitsByte11 = reader.ReadByte();
					NPC.downedPirates = bitsByte11[0];
					NPC.downedFrost = bitsByte11[1];
					NPC.downedGoblins = bitsByte11[2];
					Sandstorm.Happening = bitsByte11[3];
					DD2Event.Ongoing = bitsByte11[4];
					DD2Event.DownedInvasionT1 = bitsByte11[5];
					DD2Event.DownedInvasionT2 = bitsByte11[6];
					DD2Event.DownedInvasionT3 = bitsByte11[7];
					BitsByte bitsByte12 = reader.ReadByte();
					NPC.combatBookWasUsed = bitsByte12[0];
					LanternNight.ManualLanterns = bitsByte12[1];
					NPC.downedTowerSolar = bitsByte12[2];
					NPC.downedTowerVortex = bitsByte12[3];
					NPC.downedTowerNebula = bitsByte12[4];
					NPC.downedTowerStardust = bitsByte12[5];
					Main.forceHalloweenForToday = bitsByte12[6];
					Main.forceXMasForToday = bitsByte12[7];
					BitsByte bitsByte13 = reader.ReadByte();
					NPC.boughtCat = bitsByte13[0];
					NPC.boughtDog = bitsByte13[1];
					NPC.boughtBunny = bitsByte13[2];
					NPC.freeCake = bitsByte13[3];
					Main.drunkWorld = bitsByte13[4];
					NPC.downedEmpressOfLight = bitsByte13[5];
					NPC.downedQueenSlime = bitsByte13[6];
					Main.getGoodWorld = bitsByte13[7];
					BitsByte bitsByte14 = reader.ReadByte();
					Main.tenthAnniversaryWorld = bitsByte14[0];
					Main.dontStarveWorld = bitsByte14[1];
					NPC.downedDeerclops = bitsByte14[2];
					Main.notTheBeesWorld = bitsByte14[3];
					Main.remixWorld = bitsByte14[4];
					NPC.unlockedSlimeBlueSpawn = bitsByte14[5];
					NPC.combatBookVolumeTwoWasUsed = bitsByte14[6];
					NPC.peddlersSatchelWasUsed = bitsByte14[7];
					BitsByte bitsByte15 = reader.ReadByte();
					NPC.unlockedSlimeGreenSpawn = bitsByte15[0];
					NPC.unlockedSlimeOldSpawn = bitsByte15[1];
					NPC.unlockedSlimePurpleSpawn = bitsByte15[2];
					NPC.unlockedSlimeRainbowSpawn = bitsByte15[3];
					NPC.unlockedSlimeRedSpawn = bitsByte15[4];
					NPC.unlockedSlimeYellowSpawn = bitsByte15[5];
					NPC.unlockedSlimeCopperSpawn = bitsByte15[6];
					Main.fastForwardTimeToDusk = bitsByte15[7];
					BitsByte bitsByte16 = reader.ReadByte();
					Main.noTrapsWorld = bitsByte16[0];
					Main.zenithWorld = bitsByte16[1];
					NPC.unlockedTruffleSpawn = bitsByte16[2];
					Main.vampireSeed = bitsByte16[3];
					Main.infectedSeed = bitsByte16[4];
					Main.teamBasedSpawnsSeed = bitsByte16[5];
					Main.skyblockWorld = bitsByte16[6];
					Main.dualDungeonsSeed = bitsByte16[7];
					BitsByte bitsByte17 = reader.ReadByte();
					WorldGen.Skyblock.lowTiles = bitsByte17[0];
					Main.forceHalloweenForever = bitsByte17[1];
					Main.forceXMasForever = bitsByte17[2];
					Main.moreLightningSeed = bitsByte17[3];
					Main.noLightningSeed = bitsByte17[4];
					Main.sundialCooldown = reader.ReadByte();
					Main.moondialCooldown = reader.ReadByte();
					WorldGen.SavedOreTiers.Copper = reader.ReadInt16();
					WorldGen.SavedOreTiers.Iron = reader.ReadInt16();
					WorldGen.SavedOreTiers.Silver = reader.ReadInt16();
					WorldGen.SavedOreTiers.Gold = reader.ReadInt16();
					WorldGen.SavedOreTiers.Cobalt = reader.ReadInt16();
					WorldGen.SavedOreTiers.Mythril = reader.ReadInt16();
					WorldGen.SavedOreTiers.Adamantite = reader.ReadInt16();
					if (num76)
					{
						Main.StartSlimeRain(announce: false);
					}
					else
					{
						Main.StopSlimeRain();
					}
					Main.invasionType = reader.ReadSByte();
					Main.LobbyId = reader.ReadUInt64();
					Sandstorm.IntendedSeverity = reader.ReadSingle();
					ExtraSpawnPointManager.Read(reader, networking: true);
					Main.dungeonX = reader.ReadInt16();
					Main.dungeonY = reader.ReadInt16();
					if (Netplay.Connection.State == 3)
					{
						Main.windSpeedCurrent = Main.windSpeedTarget;
						Netplay.Connection.State = 4;
					}
					Main.checkHalloween();
					Main.checkXMas();
				}
				break;
			case 8:
			{
				if (Main.netMode != 2)
				{
					break;
				}
				NetMessage.TrySendData(7, whoAmI);
				int num194 = reader.ReadInt32();
				int num195 = reader.ReadInt32();
				int num196 = reader.ReadByte();
				bool flag20 = true;
				if (num194 == -1 || num195 == -1)
				{
					flag20 = false;
				}
				else if (num194 < 10 || num194 > Main.maxTilesX - 10)
				{
					flag20 = false;
				}
				else if (num195 < 10 || num195 > Main.maxTilesY - 10)
				{
					flag20 = false;
				}
				bool flag21 = false;
				if (Main.teamBasedSpawnsSeed && num196 != 0)
				{
					flag21 = true;
				}
				int num197 = Netplay.GetSectionX(Main.spawnTileX) - 2;
				int num198 = Netplay.GetSectionY(Main.spawnTileY) - 1;
				int num199 = num197 + 5;
				int num200 = num198 + 3;
				if (num197 < 0)
				{
					num197 = 0;
				}
				if (num199 >= Main.maxSectionsX)
				{
					num199 = Main.maxSectionsX;
				}
				if (num198 < 0)
				{
					num198 = 0;
				}
				if (num200 >= Main.maxSectionsY)
				{
					num200 = Main.maxSectionsY;
				}
				int num201 = (num199 - num197) * (num200 - num198);
				List<Point> list = new List<Point>();
				for (int num202 = num197; num202 < num199; num202++)
				{
					for (int num203 = num198; num203 < num200; num203++)
					{
						list.Add(new Point(num202, num203));
					}
				}
				int num204 = -1;
				int num205 = -1;
				if (flag20)
				{
					num194 = Netplay.GetSectionX(num194) - 2;
					num195 = Netplay.GetSectionY(num195) - 1;
					num204 = num194 + 5;
					num205 = num195 + 3;
					if (num194 < 0)
					{
						num194 = 0;
					}
					if (num204 >= Main.maxSectionsX)
					{
						num204 = Main.maxSectionsX - 1;
					}
					if (num195 < 0)
					{
						num195 = 0;
					}
					if (num205 >= Main.maxSectionsY)
					{
						num205 = Main.maxSectionsY - 1;
					}
					for (int num206 = num194; num206 <= num204; num206++)
					{
						for (int num207 = num195; num207 <= num205; num207++)
						{
							if (num206 < num197 || num206 >= num199 || num207 < num198 || num207 >= num200)
							{
								list.Add(new Point(num206, num207));
								num201++;
							}
						}
					}
				}
				int num208 = -1;
				int num209 = -1;
				int num210 = -1;
				int num211 = -1;
				if (flag21)
				{
					Point spawnPoint2 = Point.Zero;
					if (ExtraSpawnPointManager.TryGetExtraSpawnPointForTeam(num196, out spawnPoint2))
					{
						num208 = spawnPoint2.X;
						num209 = spawnPoint2.Y;
						num208 = Netplay.GetSectionX(num208) - 2;
						num209 = Netplay.GetSectionY(num209) - 1;
						num210 = num208 + 5;
						num211 = num209 + 3;
						if (num208 < 0)
						{
							num208 = 0;
						}
						if (num210 >= Main.maxSectionsX)
						{
							num210 = Main.maxSectionsX - 1;
						}
						if (num209 < 0)
						{
							num209 = 0;
						}
						if (num211 >= Main.maxSectionsY)
						{
							num211 = Main.maxSectionsY - 1;
						}
						for (int num212 = num208; num212 <= num210; num212++)
						{
							for (int num213 = num209; num213 <= num211; num213++)
							{
								if ((num212 < num197 || num212 >= num199 || num213 < num198 || num213 >= num200) && (num212 < num194 || num212 >= num204 || num213 < num195 || num213 >= num205))
								{
									list.Add(new Point(num212, num213));
									num201++;
								}
							}
						}
					}
					else
					{
						flag21 = false;
					}
				}
				PortalHelper.SyncPortalsOnPlayerJoin(whoAmI, 1, list, out var portalSections);
				num201 += portalSections.Count;
				if (Netplay.Clients[whoAmI].State == 2)
				{
					Netplay.Clients[whoAmI].State = 3;
				}
				NetMessage.TrySendData(9, whoAmI, -1, Lang.inter[44].ToNetworkText(), num201);
				for (int num214 = num197; num214 < num199; num214++)
				{
					for (int num215 = num198; num215 < num200; num215++)
					{
						NetMessage.SendSection(whoAmI, num214, num215);
					}
				}
				if (flag20)
				{
					for (int num216 = num194; num216 <= num204; num216++)
					{
						for (int num217 = num195; num217 <= num205; num217++)
						{
							NetMessage.SendSection(whoAmI, num216, num217);
						}
					}
				}
				if (flag21)
				{
					for (int num218 = num208; num218 <= num210; num218++)
					{
						for (int num219 = num209; num219 <= num211; num219++)
						{
							NetMessage.SendSection(whoAmI, num218, num219);
						}
					}
				}
				for (int num220 = 0; num220 < portalSections.Count; num220++)
				{
					NetMessage.SendSection(whoAmI, portalSections[num220].X, portalSections[num220].Y);
				}
				for (int num221 = 0; num221 < 400; num221++)
				{
					if (Main.item[num221].active)
					{
						NetMessage.TrySendData(21, whoAmI, -1, null, num221);
						NetMessage.TrySendData(22, whoAmI, -1, null, num221);
					}
				}
				for (int num222 = 0; num222 < Main.maxNPCs; num222++)
				{
					if (Main.npc[num222].active)
					{
						NetMessage.TrySendData(23, whoAmI, -1, null, num222);
						NetMessage.TrySendData(54, whoAmI, -1, null, num222);
					}
				}
				for (int num223 = 0; num223 < 1000; num223++)
				{
					if (Main.projectile[num223].active && (Main.projPet[Main.projectile[num223].type] || Main.projectile[num223].netImportant))
					{
						NetMessage.TrySendData(27, whoAmI, -1, null, num223);
					}
				}
				NetManager.Instance.SendToClient(BannerSystem.NetBannersModule.WriteFullState(), whoAmI);
				NetMessage.TrySendData(57, whoAmI);
				NetMessage.TrySendData(103);
				NetMessage.TrySendData(101, whoAmI);
				NetMessage.TrySendData(136, whoAmI);
				Main.BestiaryTracker.OnPlayerJoining(whoAmI);
				CreativePowerManager.Instance.SyncThingsToJoiningPlayer(whoAmI);
				Main.PylonSystem.OnPlayerJoining(whoAmI);
				NetMessage.TrySendData(49, whoAmI);
				break;
			}
			case 9:
				if (Main.netMode == 1)
				{
					Netplay.Connection.StatusMax += reader.ReadInt32();
					Netplay.Connection.StatusText = NetworkText.Deserialize(reader).ToString();
					BitsByte bitsByte35 = reader.ReadByte();
					BitsByte serverSpecialFlags = Netplay.Connection.ServerSpecialFlags;
					serverSpecialFlags[0] = bitsByte35[0];
					serverSpecialFlags[1] = bitsByte35[1];
					Netplay.Connection.ServerSpecialFlags = serverSpecialFlags;
				}
				break;
			case 10:
				if (Main.netMode == 1)
				{
					NetMessage.DecompressTileBlock(reader.BaseStream);
				}
				break;
			case 11:
				if (Main.netMode == 1)
				{
					WorldGen.SectionTileFrame(reader.ReadInt16(), reader.ReadInt16(), reader.ReadInt16(), reader.ReadInt16());
				}
				break;
			case 12:
			{
				int num104 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num104 = whoAmI;
				}
				Player player10 = Main.player[num104];
				player10.SpawnX = reader.ReadInt16();
				player10.SpawnY = reader.ReadInt16();
				player10.respawnTimer = reader.ReadInt32();
				player10.numberOfDeathsPVE = reader.ReadInt16();
				player10.numberOfDeathsPVP = reader.ReadInt16();
				player10.team = reader.ReadByte();
				if (player10.respawnTimer > 0)
				{
					player10.dead = true;
				}
				PlayerSpawnContext playerSpawnContext = (PlayerSpawnContext)reader.ReadByte();
				player10.Spawn(playerSpawnContext);
				if (Main.netMode != 2 || Netplay.Clients[whoAmI].State < 3)
				{
					break;
				}
				if (Netplay.Clients[whoAmI].State == 3)
				{
					Netplay.Clients[whoAmI].State = 10;
					NetMessage.buffer[whoAmI].broadcast = true;
					NetMessage.SyncConnectedPlayer(whoAmI);
					bool flag12 = NetMessage.DoesPlayerSlotCountAsAHost(whoAmI);
					Main.countsAsHostForGameplay[whoAmI] = flag12;
					if (NetMessage.DoesPlayerSlotCountAsAHost(whoAmI))
					{
						NetMessage.TrySendData(139, whoAmI, -1, null, whoAmI, flag12.ToInt());
					}
					NetMessage.TrySendData(12, -1, whoAmI, null, whoAmI, (int)(byte)playerSpawnContext);
					NetMessage.TrySendData(129, whoAmI);
					NetMessage.greetPlayer(whoAmI);
					if (Main.player[num104].unlockedBiomeTorches)
					{
						NPC nPC = new NPC();
						nPC.SetDefaults(664);
						Main.BestiaryTracker.Kills.RegisterKill(nPC);
					}
				}
				else
				{
					NetMessage.TrySendData(12, -1, whoAmI, null, whoAmI, (int)(byte)playerSpawnContext);
				}
				break;
			}
			case 13:
			{
				int num121 = reader.ReadByte();
				if (num121 == Main.myPlayer && !Main.ServerSideCharacter)
				{
					break;
				}
				if (Main.netMode == 2)
				{
					num121 = whoAmI;
				}
				Player player13 = Main.player[num121];
				BitsByte bitsByte24 = reader.ReadByte();
				BitsByte bitsByte25 = reader.ReadByte();
				BitsByte bitsByte26 = reader.ReadByte();
				BitsByte bitsByte27 = reader.ReadByte();
				player13.releaseDash |= !player13.controlDash;
				player13.controlUp = bitsByte24[0];
				player13.controlDown = bitsByte24[1];
				player13.controlLeft = bitsByte24[2];
				player13.controlRight = bitsByte24[3];
				player13.controlJump = bitsByte24[4];
				player13.controlUseItem = bitsByte24[5];
				player13.direction = (bitsByte24[6] ? 1 : (-1));
				player13.controlDash = bitsByte24[7];
				if (bitsByte25[0])
				{
					player13.pulley = true;
					player13.pulleyDir = (byte)((!bitsByte25[1]) ? 1u : 2u);
				}
				else
				{
					player13.pulley = false;
				}
				player13.vortexStealthActive = bitsByte25[3];
				player13.gravDir = (bitsByte25[4] ? 1 : (-1));
				player13.TryTogglingShield(bitsByte25[5]);
				player13.ghost = bitsByte25[6];
				player13.accSnappingStoneLightUp = bitsByte27[7];
				player13.selectedItemState.Select(reader.ReadByte());
				Vector2 vector6 = reader.ReadVector2();
				Vector2 velocity3 = Vector2.Zero;
				if (bitsByte25[2])
				{
					velocity3 = reader.ReadVector2();
				}
				if (player13.unacknowledgedTeleports > 0)
				{
					vector6 = player13.position;
					velocity3 = player13.velocity;
				}
				if (Main.netMode == 1 && player13.position != Vector2.Zero)
				{
					player13.netOffset += player13.position - vector6;
					if (player13.netOffset.Length() > (float)Main.multiplayerNPCSmoothingRange)
					{
						player13.netOffset = Vector2.Zero;
					}
					if (player13.netOffset != Vector2.Zero && DebugOptions.ShowNetOffset)
					{
						using (DebugVisualizer.InPhase(DebugVisualizer.UpdatePhase.UpdateInWorld))
						{
							DebugVisualizer.World.AddLine(vector6 + player13.Size / 2f, player13.Center, Color.Red, default(Color), 20, 2f);
						}
					}
				}
				player13.position = vector6;
				player13.velocity = velocity3;
				Vector2 t = player13.position;
				if (bitsByte25[7])
				{
					player13.mount.SetMount(reader.ReadUInt16(), player13);
				}
				else
				{
					player13.mount.Dismount(player13);
				}
				if (bitsByte26[6])
				{
					player13.PotionOfReturnOriginalUsePosition = reader.ReadVector2();
					player13.PotionOfReturnHomePosition = reader.ReadVector2();
				}
				else
				{
					player13.PotionOfReturnOriginalUsePosition = null;
					player13.PotionOfReturnHomePosition = null;
				}
				player13.tryKeepingHoveringUp = bitsByte26[0];
				player13.IsVoidVaultEnabled = bitsByte26[1];
				player13.sitting.isSitting = bitsByte26[2];
				player13.downedDD2EventAnyDifficulty = bitsByte26[3];
				player13.petting.isPetting = bitsByte26[4];
				player13.petting.isPetSmall = bitsByte26[5];
				player13.tryKeepingHoveringDown = bitsByte26[7];
				player13.sleeping.SetIsSleepingAndAdjustPlayerRotation(player13, bitsByte27[0]);
				player13.autoReuseAllWeapons = bitsByte27[1];
				player13.controlDownHold = bitsByte27[2];
				player13.isOperatingAnotherEntity = bitsByte27[3];
				player13.controlUseTile = bitsByte27[4];
				player13.netCameraTarget = (bitsByte27[5] ? new Vector2?(reader.ReadVector2()) : ((Vector2?)null));
				player13.lastItemUseAttemptSuccess = bitsByte27[6];
				Utils.Swap(ref t, ref player13.position);
				if (Main.netMode == 2 && Netplay.Clients[whoAmI].State == 10)
				{
					NetMessage.TrySendData(13, -1, whoAmI, null, num121);
				}
				Utils.Swap(ref t, ref player13.position);
				break;
			}
			case 14:
			{
				int num235 = reader.ReadByte();
				int num236 = reader.ReadByte();
				if (Main.netMode != 1)
				{
					break;
				}
				bool active = Main.player[num235].active;
				if (num236 == 1)
				{
					if (!Main.player[num235].active)
					{
						Main.player[num235] = new Player();
					}
					Main.player[num235].active = true;
				}
				else
				{
					Main.player[num235].active = false;
				}
				if (active != Main.player[num235].active)
				{
					if (Main.player[num235].active)
					{
						Player.Hooks.PlayerConnect(num235);
					}
					else
					{
						Player.Hooks.PlayerDisconnect(num235);
					}
				}
				break;
			}
			case 16:
			{
				int num73 = reader.ReadByte();
				if (num73 != Main.myPlayer || Main.ServerSideCharacter)
				{
					if (Main.netMode == 2)
					{
						num73 = whoAmI;
					}
					Player player9 = Main.player[num73];
					player9.statLife = reader.ReadInt16();
					player9.statLifeMax = reader.ReadInt16();
					if (player9.statLifeMax < 20)
					{
						player9.statLifeMax = 20;
					}
					player9.dead = player9.statLife <= 0;
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(16, -1, whoAmI, null, num73);
					}
				}
				break;
			}
			case 17:
			{
				byte b13 = reader.ReadByte();
				int num153 = reader.ReadInt16();
				int num154 = reader.ReadInt16();
				short num155 = reader.ReadInt16();
				int num156 = reader.ReadByte();
				bool flag14 = num155 == 1;
				if (!WorldGen.InWorld(num153, num154, 3))
				{
					break;
				}
				if (Main.tile[num153, num154] == null)
				{
					Main.tile[num153, num154] = new Tile();
				}
				if (Main.netMode == 2)
				{
					if (!flag14)
					{
						if (b13 == 0 || b13 == 2 || b13 == 4)
						{
							Netplay.Clients[whoAmI].SpamDeleteBlock += 1f;
						}
						if (b13 == 1 || b13 == 3)
						{
							Netplay.Clients[whoAmI].SpamAddBlock += 1f;
						}
					}
					if (!Netplay.Clients[whoAmI].TileSections[Netplay.GetSectionX(num153), Netplay.GetSectionY(num154)])
					{
						flag14 = true;
					}
				}
				MapUpdateQueue.Add(num153, num154);
				bool flag15 = false;
				using (Item.DefaultAssignNewItemsToPlayer(whoAmI))
				{
					if (b13 == 0)
					{
						WorldGen.KillTile(num153, num154, flag14);
						if (Main.netMode == 1 && !flag14)
						{
							HitTile.ClearAllTilesAtThisLocation(num153, num154);
						}
					}
					if (b13 == 1)
					{
						bool forced = true;
						if (WorldGen.CheckTileBreakability2_ShouldTileSurvive(num153, num154))
						{
							flag15 = true;
							forced = false;
						}
						WorldGen.PlaceTile(num153, num154, num155, mute: false, forced, -1, num156);
					}
					if (b13 == 2)
					{
						WorldGen.KillWall(num153, num154, flag14);
					}
					if (b13 == 3)
					{
						WorldGen.PlaceWall(num153, num154, num155);
					}
					if (b13 == 4)
					{
						WorldGen.KillTile(num153, num154, flag14, effectOnly: false, noItem: true);
					}
					if (b13 == 5)
					{
						WorldGen.PlaceWire(num153, num154);
					}
					if (b13 == 6)
					{
						WorldGen.KillWire(num153, num154);
					}
					if (b13 == 7)
					{
						WorldGen.PoundTile(num153, num154);
					}
					if (b13 == 8)
					{
						WorldGen.PlaceActuator(num153, num154);
					}
					if (b13 == 9)
					{
						WorldGen.KillActuator(num153, num154);
					}
					if (b13 == 10)
					{
						WorldGen.PlaceWire2(num153, num154);
					}
					if (b13 == 11)
					{
						WorldGen.KillWire2(num153, num154);
					}
					if (b13 == 12)
					{
						WorldGen.PlaceWire3(num153, num154);
					}
					if (b13 == 13)
					{
						WorldGen.KillWire3(num153, num154);
					}
					if (b13 == 14)
					{
						WorldGen.SlopeTile(num153, num154, num155);
					}
					if (b13 == 15)
					{
						Minecart.FrameTrack(num153, num154, pound: true);
					}
					if (b13 == 16)
					{
						WorldGen.PlaceWire4(num153, num154);
					}
					if (b13 == 17)
					{
						WorldGen.KillWire4(num153, num154);
					}
					switch (b13)
					{
					case 18:
						Wiring.SetCurrentUser(whoAmI);
						Wiring.PokeLogicGate(num153, num154);
						Wiring.SetCurrentUser();
						return;
					case 19:
						Wiring.SetCurrentUser(whoAmI);
						Wiring.Actuate(num153, num154);
						Wiring.SetCurrentUser();
						return;
					case 20:
						if (WorldGen.InWorld(num153, num154, 2))
						{
							int type16 = Main.tile[num153, num154].type;
							WorldGen.KillTile(num153, num154, flag14);
							num155 = (short)((Main.tile[num153, num154].active() && Main.tile[num153, num154].type == type16) ? 1 : 0);
							if (Main.netMode == 2)
							{
								NetMessage.TrySendData(17, -1, -1, null, b13, num153, num154, num155, num156);
							}
						}
						return;
					case 21:
						WorldGen.ReplaceTile(num153, num154, (ushort)num155, num156);
						break;
					}
					if (b13 == 22)
					{
						WorldGen.ReplaceWall(num153, num154, (ushort)num155);
					}
					if (b13 == 23 && WorldGen.CanPoundTile(num153, num154))
					{
						Main.tile[num153, num154].slope((byte)num155);
						WorldGen.PoundTile(num153, num154);
					}
				}
				if (Main.netMode == 2)
				{
					if (flag15)
					{
						NetMessage.SendTileSquare(-1, num153, num154, 5);
					}
					else if ((b13 != 1 && b13 != 21) || !TileID.Sets.Falling[num155] || Main.tile[num153, num154].active())
					{
						NetMessage.TrySendData(17, -1, whoAmI, null, b13, num153, num154, num155, num156);
					}
				}
				break;
			}
			case 18:
				if (Main.netMode == 1)
				{
					Main.dayTime = reader.ReadByte() == 1;
					Main.time = reader.ReadInt32();
					Main.sunModY = reader.ReadInt16();
					Main.moonModY = reader.ReadInt16();
				}
				break;
			case 19:
			{
				byte b5 = reader.ReadByte();
				int num46 = reader.ReadInt16();
				int num47 = reader.ReadInt16();
				if (WorldGen.InWorld(num46, num47, 3))
				{
					int num48 = ((reader.ReadByte() != 0) ? 1 : (-1));
					switch (b5)
					{
					case 0:
						WorldGen.OpenDoor(num46, num47, num48);
						break;
					case 1:
						WorldGen.CloseDoor(num46, num47, forced: true);
						break;
					case 2:
						WorldGen.ShiftTrapdoor(num46, num47, num48 == 1, 1);
						break;
					case 3:
						WorldGen.ShiftTrapdoor(num46, num47, num48 == 1, 0);
						break;
					case 4:
						WorldGen.ShiftTallGate(num46, num47, closing: false, forced: true);
						break;
					case 5:
						WorldGen.ShiftTallGate(num46, num47, closing: true, forced: true);
						break;
					}
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(19, -1, whoAmI, null, b5, num46, num47, (num48 == 1) ? 1 : 0);
					}
				}
				break;
			}
			case 20:
			{
				int num91 = reader.ReadInt16();
				int num92 = reader.ReadInt16();
				ushort num93 = reader.ReadByte();
				ushort num94 = reader.ReadByte();
				byte b7 = reader.ReadByte();
				if (!WorldGen.InWorld(num91, num92, 3))
				{
					break;
				}
				TileChangeType type7 = TileChangeType.None;
				if (Enum.IsDefined(typeof(TileChangeType), b7))
				{
					type7 = (TileChangeType)b7;
				}
				if (MessageBuffer.OnTileChangeReceived != null)
				{
					MessageBuffer.OnTileChangeReceived(num91, num92, Math.Max(num93, num94), type7);
				}
				BitsByte bitsByte19 = (byte)0;
				BitsByte bitsByte20 = (byte)0;
				BitsByte bitsByte21 = (byte)0;
				Tile tile4 = null;
				for (int num95 = num91; num95 < num91 + num93; num95++)
				{
					for (int num96 = num92; num96 < num92 + num94; num96++)
					{
						if (Main.tile[num95, num96] == null)
						{
							Main.tile[num95, num96] = new Tile();
						}
						tile4 = Main.tile[num95, num96];
						bool flag8 = tile4.active();
						bitsByte19 = reader.ReadByte();
						bitsByte20 = reader.ReadByte();
						bitsByte21 = reader.ReadByte();
						tile4.active(bitsByte19[0]);
						tile4.wall = (byte)(bitsByte19[2] ? 1u : 0u);
						bool flag9 = bitsByte19[3];
						if (Main.netMode != 2)
						{
							tile4.liquid = (byte)(flag9 ? 1u : 0u);
						}
						tile4.wire(bitsByte19[4]);
						tile4.halfBrick(bitsByte19[5]);
						tile4.actuator(bitsByte19[6]);
						tile4.inActive(bitsByte19[7]);
						tile4.wire2(bitsByte20[0]);
						tile4.wire3(bitsByte20[1]);
						if (bitsByte20[2])
						{
							tile4.color(reader.ReadByte());
						}
						if (bitsByte20[3])
						{
							tile4.wallColor(reader.ReadByte());
						}
						if (tile4.active())
						{
							int type8 = tile4.type;
							tile4.type = reader.ReadUInt16();
							if (Main.tileFrameImportant[tile4.type])
							{
								tile4.frameX = reader.ReadInt16();
								tile4.frameY = reader.ReadInt16();
							}
							else if (!flag8 || tile4.type != type8)
							{
								tile4.frameX = -1;
								tile4.frameY = -1;
							}
							byte b8 = 0;
							if (bitsByte20[4])
							{
								b8++;
							}
							if (bitsByte20[5])
							{
								b8 += 2;
							}
							if (bitsByte20[6])
							{
								b8 += 4;
							}
							tile4.slope(b8);
						}
						tile4.wire4(bitsByte20[7]);
						tile4.fullbrightBlock(bitsByte21[0]);
						tile4.fullbrightWall(bitsByte21[1]);
						tile4.invisibleBlock(bitsByte21[2]);
						tile4.invisibleWall(bitsByte21[3]);
						if (tile4.wall > 0)
						{
							tile4.wall = reader.ReadUInt16();
						}
						if (flag9)
						{
							tile4.liquid = reader.ReadByte();
							tile4.liquidType(reader.ReadByte());
						}
					}
				}
				WorldGen.RangeFrame(num91, num92, num91 + num93, num92 + num94);
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(b, -1, whoAmI, null, num91, num92, (int)num93, (int)num94, b7);
				}
				break;
			}
			case 21:
			case 90:
			{
				int num41 = reader.ReadInt16();
				Vector2 vector = reader.ReadVector2();
				Vector2 velocity = reader.ReadVector2();
				int stack3 = reader.ReadInt16();
				int num42 = reader.ReadByte();
				BitsByte bitsByte4 = reader.ReadByte();
				bool num43 = bitsByte4[2];
				bool flag3 = bitsByte4[3];
				int num44 = reader.ReadInt16();
				bool shimmered = num43 && reader.ReadBoolean();
				float shimmerTime = (num43 ? reader.ReadSingle() : 0f);
				int enemyGrabDelayTime = (flag3 ? reader.ReadByte() : 0);
				WorldItem worldItem = Main.item[num41];
				if (Main.netMode == 1)
				{
					if (worldItem.IsAir)
					{
						WorldItem[] item2 = Main.item;
						int num45 = num41;
						WorldItem obj = new WorldItem(new Item(num44))
						{
							whoAmI = num41
						};
						worldItem = obj;
						item2[num45] = obj;
					}
					else if (worldItem.type != num44)
					{
						worldItem.inner.SetDefaults(num44);
					}
					if (worldItem.prefix != num42)
					{
						worldItem.Prefix(num42);
					}
					worldItem.stack = stack3;
					worldItem.position = vector;
					worldItem.velocity = velocity;
					worldItem.shimmered = shimmered;
					worldItem.shimmerTime = shimmerTime;
					worldItem.enemyGrabDelayTime = enemyGrabDelayTime;
					worldItem.wet = Collision.WetCollision(worldItem.position, worldItem.width, worldItem.height);
					if (b == 90)
					{
						worldItem.instanced = true;
						worldItem.playerIndexTheItemIsReservedFor = Main.myPlayer;
					}
				}
				else
				{
					if (Main.timeItemSlotCannotBeReusedFor[num41] > 0)
					{
						break;
					}
					NewItemOwnership owner = (NewItemOwnership)((byte)bitsByte4 & 3);
					bool flag4 = num41 == 400;
					if (flag4)
					{
						num41 = Item.NewItem(new EntitySource_Sync(), vector + new Vector2(8f, 8f), num44, stack3, 0, NewItemOwnership.None, null, null, noBroadcast: true);
						worldItem = Main.item[num41];
					}
					else
					{
						if (worldItem.IsAir || worldItem.playerIndexTheItemIsReservedFor != whoAmI)
						{
							break;
						}
						if (num44 != worldItem.type)
						{
							worldItem.inner.SetDefaults(num44);
						}
					}
					if (num42 != worldItem.prefix)
					{
						worldItem.Prefix(num42);
					}
					worldItem.stack = stack3;
					worldItem.position = vector;
					worldItem.velocity = velocity;
					worldItem.shimmered = shimmered;
					worldItem.shimmerTime = shimmerTime;
					worldItem.enemyGrabDelayTime = enemyGrabDelayTime;
					NetMessage.TrySendData(b, -1, flag4 ? (-1) : whoAmI, null, num41);
					if (flag4)
					{
						worldItem.ApplySpawnOwnership(owner, whoAmI);
					}
				}
				break;
			}
			case 151:
			{
				int num140 = reader.ReadInt16();
				WorldItem worldItem3 = Main.item[num140];
				if ((Main.netMode != 2 || Main.timeItemSlotCannotBeReusedFor[num140] <= 0) && (Main.netMode != 2 || worldItem3.playerIndexTheItemIsReservedFor == whoAmI))
				{
					worldItem3.playerIndexTheItemIsReservedFor = 255;
					worldItem3.TurnToAir();
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(151, -1, whoAmI, null, num140);
					}
				}
				break;
			}
			case 22:
				if (Main.netMode != 2)
				{
					WorldItem obj6 = Main.item[reader.ReadInt16()];
					obj6.playerIndexTheItemIsReservedFor = reader.ReadByte();
					obj6.timeToKeepReservation = reader.Read7BitEncodedInt();
					obj6.grabDelayPlayer = reader.ReadByte();
					obj6.grabDelayTime = reader.Read7BitEncodedInt();
					obj6.position = reader.ReadVector2();
				}
				break;
			case 23:
			{
				if (Main.netMode != 1)
				{
					break;
				}
				byte b14 = reader.ReadByte();
				byte b15 = reader.ReadByte();
				Vector2 vector7 = reader.ReadVector2();
				Vector2 velocity6 = reader.ReadVector2();
				int num175 = reader.ReadUInt16();
				BitsByte bitsByte30 = reader.ReadByte();
				BitsByte bitsByte31 = reader.ReadByte();
				float[] array2 = ReUseTemporaryNPCAI();
				for (int num176 = 0; num176 < NPC.maxAI; num176++)
				{
					if (bitsByte30[num176 + 2])
					{
						array2[num176] = reader.ReadSingle();
					}
					else
					{
						array2[num176] = 0f;
					}
				}
				int num177 = reader.ReadInt16();
				int? playerCountForMultiplayerDifficultyOverride = 1;
				if (bitsByte31[0])
				{
					playerCountForMultiplayerDifficultyOverride = reader.ReadByte();
				}
				float value4 = 1f;
				if (bitsByte31[2])
				{
					value4 = reader.ReadSingle();
				}
				int num178 = 0;
				if (!bitsByte30[7])
				{
					switch (reader.ReadByte())
					{
					case 2:
						num178 = reader.ReadInt16();
						break;
					case 4:
						num178 = reader.ReadInt32();
						break;
					default:
						num178 = reader.ReadSByte();
						break;
					}
				}
				NPC nPC5 = Main.npc[b14];
				bool flag17 = bitsByte31[3] || nPC5.generation != b15;
				int num179 = -1;
				if (flag17)
				{
					nPC5 = NPC.NewNPCInstanceInSlot(b14, b15);
					nPC5.SetDefaults(num177, new NPCSpawnParams
					{
						playerCountForMultiplayerDifficultyOverride = playerCountForMultiplayerDifficultyOverride,
						difficultyOverride = value4
					});
				}
				else if (nPC5.netID != num177)
				{
					num179 = nPC5.type;
					nPC5.active = true;
					nPC5.SetDefaults(num177, new NPCSpawnParams
					{
						playerCountForMultiplayerDifficultyOverride = playerCountForMultiplayerDifficultyOverride,
						difficultyOverride = value4
					});
				}
				else if (!nPC5.active)
				{
					nPC5.active = true;
				}
				Vector2 vector8 = NPCID.Sets.SyncAnchor[nPC5.type] * nPC5.Size;
				if (!flag17 && Vector2.DistanceSquared(nPC5.position + vector8, vector7) <= (float)(Main.multiplayerNPCSmoothingRange * Main.multiplayerNPCSmoothingRange))
				{
					nPC5.netOffset += nPC5.position + vector8 - vector7;
					if (nPC5.netOffset != Vector2.Zero && DebugOptions.ShowNetOffset)
					{
						using (DebugVisualizer.InPhase(DebugVisualizer.UpdatePhase.UpdateInWorld))
						{
							DebugVisualizer.World.AddLine(vector7 + nPC5.Size / 2f, nPC5.Center, Color.Red, default(Color), 20, 2f);
						}
					}
				}
				nPC5.position = vector7 - vector8;
				nPC5.velocity = velocity6;
				if (nPC5.target != num175)
				{
					nPC5.targetSetFrame = Main.EverLastingTicker;
				}
				nPC5.target = num175;
				nPC5.direction = (bitsByte30[0] ? 1 : (-1));
				nPC5.directionY = (bitsByte30[1] ? 1 : (-1));
				nPC5.spriteDirection = (bitsByte30[6] ? 1 : (-1));
				if (bitsByte30[7])
				{
					num178 = nPC5.lifeMax;
				}
				if (num178 <= 0)
				{
					nPC5.life = num178;
					nPC5.active = false;
				}
				else
				{
					NPC.GetPendingDamage(nPC5, out var damage4, out var phaseChange);
					if (!phaseChange)
					{
						nPC5.life = num178 - damage4;
						for (int num180 = 0; num180 < NPC.maxAI; num180++)
						{
							nPC5.ai[num180] = array2[num180];
						}
					}
				}
				if (num175 == 65535 && nPC5.active)
				{
					nPC5.target = 0;
					Invariant.Assert(condition: false, "npc ({0}) had invalid target -1", nPC5);
				}
				nPC5.SpawnedFromStatue = bitsByte31[1];
				if (nPC5.SpawnedFromStatue)
				{
					nPC5.value = 0f;
				}
				if (bitsByte31[4])
				{
					nPC5.shimmerTransparency = 1f;
				}
				if (num179 > -1)
				{
					nPC5.TransformVisuals(num179, nPC5.type);
				}
				if (nPC5.type >= 0 && nPC5.type < NPCID.Count && Main.npcCatchable[nPC5.type])
				{
					nPC5.releaseOwner = reader.ReadByte();
				}
				if (flag17)
				{
					nPC5.OnSpawn(new EntitySource_Sync());
				}
				break;
			}
			case 24:
				Invariant.Assert(condition: false, "UnusedMeleeStrike");
				break;
			case 27:
			{
				ProjectileKey key2 = (ProjectileKey)reader.ReadInt32();
				Vector2 position4 = reader.ReadVector2();
				Vector2 velocity5 = reader.ReadVector2();
				int num158 = reader.ReadInt16();
				BitsByte bitsByte28 = reader.ReadByte();
				BitsByte bitsByte29 = (byte)(bitsByte28[2] ? reader.ReadByte() : 0);
				float[] array = ReUseTemporaryProjectileAI();
				array[0] = (bitsByte28[0] ? reader.ReadSingle() : 0f);
				array[1] = (bitsByte28[1] ? reader.ReadSingle() : 0f);
				int bannerIdToRespondTo = (bitsByte28[3] ? reader.ReadUInt16() : 0);
				int damage3 = (bitsByte28[4] ? reader.ReadInt16() : 0);
				float knockBack2 = (bitsByte28[5] ? reader.ReadSingle() : 0f);
				int originalDamage = (bitsByte28[6] ? reader.ReadInt16() : 0);
				array[2] = (bitsByte29[0] ? reader.ReadSingle() : 0f);
				if (Main.netMode == 2 && (Main.projHostile[num158] || key2.Spawner != whoAmI))
				{
					break;
				}
				bool flag16 = false;
				if (!key2.TryGet(out var proj2))
				{
					flag16 = true;
					proj2 = Projectile.NewProjectileSetup(key2);
					proj2.SetDefaults(num158);
					if (Main.netMode == 2)
					{
						Netplay.Clients[whoAmI].SpamProjectile += 1f;
					}
				}
				else if (num158 != proj2.type)
				{
					proj2.SetDefaults(num158);
				}
				proj2.owner = key2.Spawner;
				proj2.position = position4;
				proj2.velocity = velocity5;
				proj2.type = num158;
				proj2.damage = damage3;
				proj2.bannerIdToRespondTo = bannerIdToRespondTo;
				proj2.originalDamage = originalDamage;
				proj2.knockBack = knockBack2;
				for (int num159 = 0; num159 < Projectile.maxAI; num159++)
				{
					proj2.ai[num159] = array[num159];
				}
				if (flag16)
				{
					proj2.FinalizeProjectile();
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(27, -1, whoAmI, null, proj2.whoAmI);
				}
				break;
			}
			case 28:
			{
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(162, whoAmI);
				}
				int num240 = reader.ReadByte();
				int num241 = reader.ReadByte();
				int num242 = reader.ReadInt16();
				float num243 = reader.ReadSingle();
				int num244 = reader.ReadByte() - 1;
				byte b18 = reader.ReadByte();
				NPC nPC7 = Main.npc[num240];
				if (Main.netMode == 2)
				{
					if (nPC7.generation != num241)
					{
						break;
					}
					if (num242 < 0)
					{
						num242 = 0;
					}
					nPC7.PlayerInteraction(whoAmI);
				}
				else
				{
					Invariant.Assert(nPC7.generation == num241, "NPC Generation desync slot: {0} type: {1}", num240, num241);
				}
				if (num242 >= 0)
				{
					nPC7.StrikeNPC(num242, num243, num244, b18 == 1, fromNet: true, (Main.netMode == 2) ? whoAmI : 255);
				}
				else
				{
					nPC7.life = 0;
					nPC7.HitEffect();
					nPC7.active = false;
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(28, -1, whoAmI, null, num240, num242, num243, num244, b18);
					if (nPC7.life <= 0)
					{
						NetMessage.TrySendData(23, -1, -1, null, num240);
					}
					if (nPC7.realLife >= 0 && Main.npc[nPC7.realLife].life <= 0)
					{
						NetMessage.TrySendData(23, -1, -1, null, nPC7.realLife);
					}
				}
				break;
			}
			case 162:
				if (Main.netMode == 1)
				{
					NPC.AckDamage();
				}
				break;
			case 29:
			{
				ProjectileKey projectileKey = (ProjectileKey)reader.ReadInt32();
				Vector2 position3 = reader.ReadVector2();
				if (projectileKey.TryGet(out var proj) && proj.active)
				{
					if (Main.netMode == 2 && proj.owner != whoAmI)
					{
						break;
					}
					if (!float.IsInfinity(position3.X) && !float.IsNaN(position3.X) && !float.IsInfinity(position3.Y) && !float.IsNaN(position3.Y))
					{
						proj.position = position3;
						proj.Kill();
					}
					else
					{
						proj.active = false;
					}
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(29, -1, whoAmI, null, projectileKey, position3.X, position3.Y);
				}
				break;
			}
			case 30:
			{
				int num52 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num52 = whoAmI;
				}
				bool flag5 = reader.ReadBoolean();
				Main.player[num52].hostile = flag5;
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(30, -1, whoAmI, null, num52);
					LocalizedText obj2 = (flag5 ? Lang.mp[11] : Lang.mp[12]);
					ChatHelper.BroadcastChatMessage(color: Main.teamColor[Main.player[num52].team], text: NetworkText.FromKey(obj2.Key, Main.player[num52].name));
				}
				break;
			}
			case 31:
			{
				if (Main.netMode != 2)
				{
					break;
				}
				int num255 = reader.ReadInt16();
				int num256 = reader.ReadInt16();
				int num257 = Chest.FindChest(num255, num256);
				if (num257 > -1 && Chest.UsingChest(num257) == -1)
				{
					NetMessage.SendChestContentsTo(num257, whoAmI);
					NetMessage.TrySendData(33, whoAmI, -1, null, num257);
					Main.player[whoAmI].chest = num257;
					if (Main.myPlayer == whoAmI)
					{
						Main.PipsUseGrid = false;
					}
					NetMessage.TrySendData(80, -1, whoAmI, null, whoAmI, num257);
					if (Main.netMode == 2 && WorldGen.IsChestRigged(num255, num256))
					{
						Wiring.SetCurrentUser(whoAmI);
						Wiring.HitSwitch(num255, num256);
						Wiring.SetCurrentUser();
						NetMessage.TrySendData(59, -1, whoAmI, null, num255, num256);
					}
				}
				break;
			}
			case 32:
			{
				int num149 = reader.ReadInt16();
				int num150 = reader.ReadByte();
				int stack7 = reader.ReadInt16();
				int prefixWeWant3 = reader.ReadByte();
				int type15 = reader.ReadInt16();
				if (num149 >= 0 && num149 < 8000 && Main.chest[num149] != null)
				{
					if (Main.chest[num149].item[num150] == null)
					{
						Main.chest[num149].item[num150] = new Item();
					}
					Main.chest[num149].item[num150].SetDefaults(type15);
					Main.chest[num149].item[num150].Prefix(prefixWeWant3);
					Main.chest[num149].item[num150].stack = stack7;
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(32, -1, whoAmI, null, num149, num150);
					}
				}
				break;
			}
			case 33:
			{
				int num21 = reader.ReadInt16();
				int num22 = reader.ReadInt16();
				int num23 = reader.ReadInt16();
				int num24 = reader.ReadByte();
				string name = string.Empty;
				if (num24 != 0)
				{
					if (num24 <= 20)
					{
						name = reader.ReadString();
					}
					else if (num24 != 255)
					{
						num24 = 0;
					}
				}
				if (Main.netMode == 1)
				{
					Player player = Main.player[Main.myPlayer];
					if (player.chest == -1)
					{
						Main.playerInventory = true;
						SoundEngine.PlaySound(10);
						if (num21 != -1)
						{
							ItemSlot.SetGlowForChest(Main.chest[num21]);
						}
					}
					else if (player.chest != num21 && num21 != -1)
					{
						Main.playerInventory = true;
						SoundEngine.PlaySound(12);
						Main.PipsUseGrid = false;
						ItemSlot.SetGlowForChest(Main.chest[num21]);
					}
					else if (player.chest != -1 && num21 == -1)
					{
						SoundEngine.PlaySound(11);
						Main.PipsUseGrid = false;
					}
					player.chest = num21;
					player.chestX = num22;
					player.chestY = num23;
					if (Main.tile[num22, num23].frameX >= 36 && Main.tile[num22, num23].frameX < 72)
					{
						AchievementsHelper.HandleSpecialEvent(Main.player[Main.myPlayer], 16);
					}
				}
				else
				{
					if (num24 != 0)
					{
						int chest3 = Main.player[whoAmI].chest;
						Chest chest4 = Main.chest[chest3];
						chest4.name = name;
						NetMessage.TrySendData(69, -1, whoAmI, null, chest3, chest4.x, chest4.y);
					}
					Main.player[whoAmI].chest = num21;
					NetMessage.TrySendData(80, -1, whoAmI, null, whoAmI, num21);
				}
				break;
			}
			case 34:
			{
				byte b4 = reader.ReadByte();
				int num32 = reader.ReadInt16();
				int num33 = reader.ReadInt16();
				int num34 = reader.ReadInt16();
				int num35 = reader.ReadInt16();
				if (Main.netMode == 2)
				{
					num35 = 0;
				}
				if (Main.netMode == 2)
				{
					using (Item.DefaultAssignNewItemsToPlayer(whoAmI))
					{
						switch (b4)
						{
						case 0:
						{
							int num38 = WorldGen.PlaceChest(num32, num33, 21, notNearOtherChests: false, num34);
							if (num38 == -1)
							{
								NetMessage.TrySendData(34, whoAmI, -1, null, b4, num32, num33, num34, num38);
								int itemDrop_Chests2 = WorldGen.GetItemDrop_Chests(num34, secondType: false);
								if (itemDrop_Chests2 > 0)
								{
									Item.NewItem(new EntitySource_TileBreak(num32, num33), num32 * 16, num33 * 16, 32, 32, itemDrop_Chests2, 1, noBroadcast: true);
								}
							}
							else
							{
								NetMessage.TrySendData(34, -1, -1, null, b4, num32, num33, num34, num38);
							}
							break;
						}
						case 1:
							if (Main.tile[num32, num33].type == 21)
							{
								Tile tile = Main.tile[num32, num33];
								if (tile.frameX % 36 != 0)
								{
									num32--;
								}
								if (tile.frameY % 36 != 0)
								{
									num33--;
								}
								int number = Chest.FindChest(num32, num33);
								WorldGen.KillTile(num32, num33);
								if (!tile.active())
								{
									NetMessage.TrySendData(34, -1, -1, null, b4, num32, num33, 0f, number);
								}
								break;
							}
							goto default;
						default:
							switch (b4)
							{
							case 2:
							{
								int num36 = WorldGen.PlaceChest(num32, num33, 88, notNearOtherChests: false, num34);
								if (num36 == -1)
								{
									NetMessage.TrySendData(34, whoAmI, -1, null, b4, num32, num33, num34, num36);
									Item.NewItem(new EntitySource_TileBreak(num32, num33), num32 * 16, num33 * 16, 32, 32, WorldGen.GetItemDrop_Dressers(num34), 1, noBroadcast: true);
								}
								else
								{
									NetMessage.TrySendData(34, -1, -1, null, b4, num32, num33, num34, num36);
								}
								break;
							}
							case 3:
								if (Main.tile[num32, num33].type == 88)
								{
									Tile tile2 = Main.tile[num32, num33];
									num32 -= tile2.frameX % 54 / 18;
									if (tile2.frameY % 36 != 0)
									{
										num33--;
									}
									int number2 = Chest.FindChest(num32, num33);
									WorldGen.KillTile(num32, num33);
									if (!tile2.active())
									{
										NetMessage.TrySendData(34, -1, -1, null, b4, num32, num33, 0f, number2);
									}
									break;
								}
								goto default;
							default:
								switch (b4)
								{
								case 4:
								{
									int num37 = WorldGen.PlaceChest(num32, num33, 467, notNearOtherChests: false, num34);
									if (num37 == -1)
									{
										NetMessage.TrySendData(34, whoAmI, -1, null, b4, num32, num33, num34, num37);
										int itemDrop_Chests = WorldGen.GetItemDrop_Chests(num34, secondType: true);
										if (itemDrop_Chests > 0)
										{
											Item.NewItem(new EntitySource_TileBreak(num32, num33), num32 * 16, num33 * 16, 32, 32, itemDrop_Chests, 1, noBroadcast: true);
										}
									}
									else
									{
										NetMessage.TrySendData(34, -1, -1, null, b4, num32, num33, num34, num37);
									}
									break;
								}
								case 5:
									if (Main.tile[num32, num33].type == 467)
									{
										Tile tile3 = Main.tile[num32, num33];
										if (tile3.frameX % 36 != 0)
										{
											num32--;
										}
										if (tile3.frameY % 36 != 0)
										{
											num33--;
										}
										int number3 = Chest.FindChest(num32, num33);
										WorldGen.KillTile(num32, num33);
										if (!tile3.active())
										{
											NetMessage.TrySendData(34, -1, -1, null, b4, num32, num33, 0f, number3);
										}
									}
									break;
								}
								break;
							}
							break;
						}
						break;
					}
				}
				switch (b4)
				{
				case 0:
					if (num35 == -1)
					{
						WorldGen.KillTile(num32, num33);
						break;
					}
					SoundEngine.PlaySound(0, num32 * 16, num33 * 16);
					WorldGen.PlaceChestDirect(num32, num33, 21, num34, num35);
					break;
				case 2:
					if (num35 == -1)
					{
						WorldGen.KillTile(num32, num33);
						break;
					}
					SoundEngine.PlaySound(0, num32 * 16, num33 * 16);
					WorldGen.PlaceDresserDirect(num32, num33, 88, num34, num35);
					break;
				case 4:
					if (num35 == -1)
					{
						WorldGen.KillTile(num32, num33);
						break;
					}
					SoundEngine.PlaySound(0, num32 * 16, num33 * 16);
					WorldGen.PlaceChestDirect(num32, num33, 467, num34, num35);
					break;
				default:
					Chest.DestroyChestDirect(num32, num33, num35);
					WorldGen.KillTile(num32, num33);
					break;
				}
				break;
			}
			case 35:
			{
				int num167 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num167 = whoAmI;
				}
				int num168 = reader.ReadInt16();
				if (num167 != Main.myPlayer || Main.ServerSideCharacter)
				{
					Main.player[num167].HealEffect(num168);
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(35, -1, whoAmI, null, num167, num168);
				}
				break;
			}
			case 36:
			{
				int num131 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num131 = whoAmI;
				}
				Player player16 = Main.player[num131];
				bool flag13 = player16.zone5[0];
				player16.zone1 = reader.ReadByte();
				player16.zone2 = reader.ReadByte();
				player16.zone3 = reader.ReadByte();
				player16.zone4 = reader.ReadByte();
				player16.zone5 = reader.ReadByte();
				player16.townNPCs = reader.ReadByte();
				if (Main.netMode == 2)
				{
					if (!flag13 && player16.zone5[0])
					{
						NPC.Spawner.SpawnFaelings(player16);
					}
					NetMessage.TrySendData(36, -1, whoAmI, null, num131);
				}
				break;
			}
			case 37:
				if (Main.netMode == 1)
				{
					if (Main.autoPass)
					{
						NetMessage.TrySendData(38);
						Main.autoPass = false;
					}
					else
					{
						Netplay.ServerPassword = "";
						Main.menuMode = 31;
					}
				}
				break;
			case 38:
				if (Main.netMode == 2)
				{
					if (reader.ReadString() == Netplay.ServerPassword)
					{
						Netplay.Clients[whoAmI].State = 1;
						NetMessage.TrySendData(3, whoAmI);
					}
					else
					{
						NetMessage.TrySendData(2, whoAmI, -1, Lang.mp[1].ToNetworkText());
					}
				}
				break;
			case 39:
			{
				int num61 = reader.ReadInt16();
				WorldItem worldItem2 = Main.item[num61];
				bool forceAssignToServer = reader.ReadBoolean();
				if (Main.netMode == 1)
				{
					if (worldItem2.playerIndexTheItemIsReservedFor == Main.myPlayer)
					{
						worldItem2.FindOwner(forceAssignToServer: true);
					}
				}
				else if (worldItem2.playerIndexTheItemIsReservedFor == whoAmI)
				{
					worldItem2.timeSinceTheItemHasBeenReservedForSomeone = 0;
					worldItem2.playerIndexTheItemIsReservedFor = 255;
					worldItem2.FindOwner(forceAssignToServer);
					if (worldItem2.playerIndexTheItemIsReservedFor == 255)
					{
						NetMessage.TrySendData(22, -1, whoAmI, null, num61);
					}
				}
				break;
			}
			case 40:
			{
				int num53 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num53 = whoAmI;
				}
				int talkNPC = reader.ReadInt16();
				Main.player[num53].SetTalkNPC(talkNPC);
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(40, -1, whoAmI, null, num53);
				}
				break;
			}
			case 41:
			{
				int num30 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num30 = whoAmI;
				}
				Player player3 = Main.player[num30];
				float itemRotation = reader.ReadSingle();
				int itemAnimation = reader.ReadInt16();
				player3.itemRotation = itemRotation;
				player3.itemAnimation = itemAnimation;
				player3.channel = player3.inventory[player3.selectedItem].channel;
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(41, -1, whoAmI, null, num30);
				}
				break;
			}
			case 42:
			{
				int num245 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num245 = whoAmI;
				}
				else if (Main.myPlayer == num245 && !Main.ServerSideCharacter)
				{
					break;
				}
				int statMana = reader.ReadInt16();
				int statManaMax = reader.ReadInt16();
				Main.player[num245].statMana = statMana;
				Main.player[num245].statManaMax = statManaMax;
				break;
			}
			case 43:
			{
				int num181 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num181 = whoAmI;
				}
				int num182 = reader.ReadInt16();
				if (num181 != Main.myPlayer)
				{
					Main.player[num181].ManaEffect(num182);
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(43, -1, whoAmI, null, num181, num182);
				}
				break;
			}
			case 45:
			case 157:
			{
				int num122 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num122 = whoAmI;
				}
				int num123 = reader.ReadByte();
				Player player14 = Main.player[num122];
				int team = player14.team;
				player14.team = num123;
				Color color = Main.teamColor[num123];
				if (Main.netMode != 2)
				{
					break;
				}
				NetMessage.TrySendData(45, -1, whoAmI, null, num122);
				LocalizedText localizedText = Lang.mp[13 + num123];
				if (num123 == 5)
				{
					localizedText = Lang.mp[22];
				}
				for (int num124 = 0; num124 < 255; num124++)
				{
					if (num124 == whoAmI || (team > 0 && Main.player[num124].team == team) || (num123 > 0 && Main.player[num124].team == num123))
					{
						ChatHelper.SendChatMessageToClient(NetworkText.FromKey(localizedText.Key, player14.name), color, num124);
					}
				}
				if (b == 157 && Main.teamBasedSpawnsSeed)
				{
					Point spawnPoint = Point.Zero;
					if (ExtraSpawnPointManager.TryGetExtraSpawnPointForTeam(num123, out spawnPoint))
					{
						RemoteClient.CheckSection(whoAmI, spawnPoint.ToWorldCoordinates());
						NetMessage.SendData(158, num122, -1, null, num122);
					}
				}
				break;
			}
			case 46:
				if (Main.netMode == 2)
				{
					short i3 = reader.ReadInt16();
					int j3 = reader.ReadInt16();
					int num120 = Sign.ReadSign(i3, j3);
					if (num120 >= 0)
					{
						NetMessage.TrySendData(47, whoAmI, -1, null, num120, whoAmI);
					}
				}
				break;
			case 47:
			{
				int num59 = reader.ReadInt16();
				int x3 = reader.ReadInt16();
				int y3 = reader.ReadInt16();
				string text2 = reader.ReadString();
				int num60 = reader.ReadByte();
				BitsByte bitsByte5 = reader.ReadByte();
				if (num59 >= 0 && num59 < 32000)
				{
					string text3 = null;
					if (Main.sign[num59] != null)
					{
						text3 = Main.sign[num59].text;
					}
					Main.sign[num59] = new Sign();
					Main.sign[num59].x = x3;
					Main.sign[num59].y = y3;
					Sign.TextSign(num59, text2);
					if (Main.netMode == 2 && text3 != text2)
					{
						num60 = whoAmI;
						NetMessage.TrySendData(47, -1, whoAmI, null, num59, num60);
					}
					if (Main.netMode == 1 && num60 == Main.myPlayer && Main.sign[num59] != null && !bitsByte5[0])
					{
						Main.LocalPlayer.OpenSign(num59);
					}
				}
				break;
			}
			case 48:
			{
				int num3 = reader.ReadInt16();
				int num4 = reader.ReadInt16();
				byte b2 = reader.ReadByte();
				byte liquidType = reader.ReadByte();
				if (Main.netMode == 2 && Netplay.SpamCheck)
				{
					int num5 = whoAmI;
					int num6 = (int)(Main.player[num5].position.X + (float)(Main.player[num5].width / 2));
					int num7 = (int)(Main.player[num5].position.Y + (float)(Main.player[num5].height / 2));
					int num8 = 10;
					int num9 = num6 - num8;
					int num10 = num6 + num8;
					int num11 = num7 - num8;
					int num12 = num7 + num8;
					if (num3 < num9 || num3 > num10 || num4 < num11 || num4 > num12)
					{
						Netplay.Clients[whoAmI].SpamWater += 1f;
					}
				}
				if (Main.tile[num3, num4] == null)
				{
					Main.tile[num3, num4] = new Tile();
				}
				lock (Main.tile[num3, num4])
				{
					Main.tile[num3, num4].liquid = b2;
					Main.tile[num3, num4].liquidType(liquidType);
					if (Main.netMode == 2)
					{
						WorldGen.SquareTileFrame(num3, num4);
						if (b2 == 0)
						{
							NetMessage.SendData(48, -1, whoAmI, null, num3, num4);
						}
					}
					break;
				}
			}
			case 49:
				if (Netplay.Connection.State == 6)
				{
					Netplay.Connection.State = 10;
					Main.player[Main.myPlayer].Spawn(PlayerSpawnContext.SpawningIntoWorld);
				}
				break;
			case 50:
			{
				int num228 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num228 = whoAmI;
				}
				else if (num228 == Main.myPlayer && !Main.ServerSideCharacter)
				{
					break;
				}
				Player player19 = Main.player[num228];
				int num229 = 0;
				int num230;
				while ((num230 = reader.ReadUInt16()) > 0)
				{
					player19.buffType[num229] = num230;
					player19.buffTime[num229] = 60;
					num229++;
				}
				Array.Clear(player19.buffType, num229, player19.buffType.Length - num229);
				Array.Clear(player19.buffTime, num229, player19.buffTime.Length - num229);
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(50, -1, whoAmI, null, num228);
				}
				break;
			}
			case 51:
			{
				byte b16 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					b16 = (byte)whoAmI;
				}
				byte b17 = reader.ReadByte();
				switch (b17)
				{
				case 1:
					NPC.SpawnSkeletron(b16);
					break;
				case 2:
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(51, -1, whoAmI, null, b16, (int)b17);
					}
					else
					{
						SoundEngine.PlaySound(SoundID.Item1, (int)Main.player[b16].position.X, (int)Main.player[b16].position.Y);
					}
					break;
				case 3:
					if (Main.netMode == 2)
					{
						Main.Sundialing();
					}
					break;
				case 4:
					Main.npc[b16].BigMimicSpawnSmoke();
					break;
				case 5:
					if (Main.netMode == 2)
					{
						NPC nPC6 = new NPC();
						nPC6.SetDefaults(664);
						Main.BestiaryTracker.Kills.RegisterKill(nPC6);
					}
					break;
				case 6:
					if (Main.netMode == 2)
					{
						Main.Moondialing();
					}
					break;
				}
				break;
			}
			case 52:
			{
				int num137 = reader.ReadByte();
				int num138 = reader.ReadInt16();
				int num139 = reader.ReadInt16();
				if (num137 == 1)
				{
					Chest.Unlock(num138, num139);
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(52, -1, whoAmI, null, 0, num137, num138, num139);
						NetMessage.SendTileSquare(-1, num138, num139, 2);
					}
				}
				if (num137 == 2)
				{
					WorldGen.UnlockDoor(num138, num139);
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(52, -1, whoAmI, null, 0, num137, num138, num139);
						NetMessage.SendTileSquare(-1, num138, num139, 2);
					}
				}
				if (num137 == 3)
				{
					Chest.Lock(num138, num139);
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(52, -1, whoAmI, null, 0, num137, num138, num139);
						NetMessage.SendTileSquare(-1, num138, num139, 2);
					}
				}
				break;
			}
			case 53:
			{
				int num136 = reader.ReadInt16();
				int type13 = reader.ReadUInt16();
				int time2 = reader.ReadInt16();
				Main.npc[num136].AddBuff(type13, time2, quiet: true);
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(54, -1, -1, null, num136);
				}
				break;
			}
			case 54:
				if (Main.netMode == 1)
				{
					int num125 = reader.ReadInt16();
					NPC nPC2 = Main.npc[num125];
					int num126 = 0;
					int num127;
					while ((num127 = reader.ReadUInt16()) > 0)
					{
						nPC2.buffType[num126] = num127;
						nPC2.buffTime[num126] = reader.ReadUInt16();
						num126++;
					}
					Array.Clear(nPC2.buffType, num126, nPC2.buffType.Length - num126);
					Array.Clear(nPC2.buffTime, num126, nPC2.buffTime.Length - num126);
				}
				break;
			case 55:
			{
				int num78 = reader.ReadByte();
				int num79 = reader.ReadUInt16();
				int num80 = reader.ReadInt32();
				if ((Main.netMode != 2 || (Main.player[num78].hostile && Main.player[whoAmI].hostile && Main.pvpBuff[num79])) && (Main.netMode != 1 || num78 == Main.myPlayer))
				{
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(55, num78, -1, null, num78, num79, num80);
					}
					else
					{
						Main.player[num78].AddBuff(num79, num80, fromNetPvP: true);
					}
				}
				break;
			}
			case 56:
			{
				int num56 = reader.ReadInt16();
				if (num56 >= 0 && num56 < Main.maxNPCs)
				{
					if (Main.netMode == 1)
					{
						string givenName = reader.ReadString();
						Main.npc[num56].GivenName = givenName;
						int townNpcVariationIndex = reader.ReadInt32();
						Main.npc[num56].townNpcVariationIndex = townNpcVariationIndex;
					}
					else if (Main.netMode == 2)
					{
						NetMessage.TrySendData(56, whoAmI, -1, null, num56);
					}
				}
				break;
			}
			case 57:
				if (Main.netMode == 1)
				{
					WorldGen.tGood = reader.ReadByte();
					WorldGen.tEvil = reader.ReadByte();
					WorldGen.tBlood = reader.ReadByte();
				}
				break;
			case 58:
			{
				int num39 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num39 = whoAmI;
				}
				float num40 = reader.ReadSingle();
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(58, -1, whoAmI, null, whoAmI, num40);
					break;
				}
				Player player4 = Main.player[num39];
				int type3 = player4.inventory[player4.selectedItem].type;
				switch (type3)
				{
				case 4057:
				case 4372:
				case 4715:
					player4.PlayGuitarChord(num40);
					break;
				case 4673:
					player4.PlayDrums(num40);
					break;
				default:
				{
					Main.musicPitch = num40;
					LegacySoundStyle type4 = SoundID.Item26;
					if (type3 == 507)
					{
						type4 = SoundID.Item35;
					}
					if (type3 == 1305)
					{
						type4 = SoundID.Item47;
					}
					SoundEngine.PlaySound(type4, player4.position);
					break;
				}
				}
				break;
			}
			case 59:
			{
				int num54 = reader.ReadInt16();
				int num55 = reader.ReadInt16();
				Wiring.SetCurrentUser(whoAmI);
				Wiring.HitSwitch(num54, num55);
				Wiring.SetCurrentUser();
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(59, -1, whoAmI, null, num54, num55);
				}
				break;
			}
			case 60:
			{
				int num261 = reader.ReadInt16();
				int num262 = reader.ReadInt16();
				int num263 = reader.ReadInt16();
				byte b19 = reader.ReadByte();
				if (num261 >= Main.maxNPCs)
				{
					NetMessage.BootPlayer(whoAmI, NetworkText.FromKey("Net.CheatingInvalid"));
					break;
				}
				NPC nPC8 = Main.npc[num261];
				bool isLikeATownNPC = nPC8.isLikeATownNPC;
				if (Main.netMode == 1)
				{
					nPC8.homeless = b19 == 1;
					nPC8.homeTileX = num262;
					nPC8.homeTileY = num263;
				}
				if (!isLikeATownNPC)
				{
					break;
				}
				if (Main.netMode == 1)
				{
					switch (b19)
					{
					case 1:
						WorldGen.TownManager.KickOut(nPC8.type);
						break;
					case 2:
						WorldGen.TownManager.SetRoom(nPC8.type, num262, num263);
						break;
					}
				}
				else if (b19 == 1)
				{
					WorldGen.kickOut(num261);
				}
				else
				{
					WorldGen.moveRoom(num262, num263, num261);
				}
				break;
			}
			case 61:
			{
				int num237 = reader.ReadInt16();
				int num238 = reader.ReadInt16();
				if (Main.netMode != 2)
				{
					break;
				}
				if (num238 >= 0 && num238 < NPCID.Count && NPCID.Sets.MPAllowedEnemies[num238])
				{
					if (!NPC.AnyNPCs(num238))
					{
						NPC.SpawnOnPlayer(num237, num238);
					}
				}
				else if (num238 == -4)
				{
					if (!Main.dayTime && !DD2Event.Ongoing)
					{
						ChatHelper.BroadcastChatMessage(NetworkText.FromKey(Lang.misc[31].Key), ChatColors.World);
						Main.startPumpkinMoon();
						NetMessage.TrySendData(7);
						NetMessage.TrySendData(78, -1, -1, null, 0, 1f, 2f, 1f);
					}
				}
				else if (num238 == -5)
				{
					if (!Main.dayTime && !DD2Event.Ongoing)
					{
						ChatHelper.BroadcastChatMessage(NetworkText.FromKey(Lang.misc[34].Key), ChatColors.World);
						Main.startSnowMoon();
						NetMessage.TrySendData(7);
						NetMessage.TrySendData(78, -1, -1, null, 0, 1f, 1f, 1f);
					}
				}
				else if (num238 == -6)
				{
					if (Main.dayTime && !Main.eclipse)
					{
						if (Main.remixWorld)
						{
							ChatHelper.BroadcastChatMessage(NetworkText.FromKey(Lang.misc[106].Key), ChatColors.World);
						}
						else
						{
							ChatHelper.BroadcastChatMessage(NetworkText.FromKey(Lang.misc[20].Key), ChatColors.World);
						}
						Main.eclipse = true;
						NetMessage.TrySendData(7);
					}
				}
				else if (num238 == -7)
				{
					Main.invasionDelay = 0;
					Main.StartInvasion(4);
					NetMessage.TrySendData(7);
					NetMessage.TrySendData(78, -1, -1, null, 0, 1f, Main.invasionType + 3);
				}
				else if (num238 == -8)
				{
					if (NPC.downedGolemBoss && Main.hardMode && !NPC.AnyDanger() && !NPC.AnyoneNearCultists())
					{
						WorldGen.StartImpendingDoom(720);
						NetMessage.TrySendData(7);
					}
				}
				else if (num238 == -10)
				{
					if (!Main.dayTime && !Main.bloodMoon)
					{
						ChatHelper.BroadcastChatMessage(NetworkText.FromKey(Lang.misc[8].Key), ChatColors.World);
						Main.bloodMoon = true;
						if (Main.GetMoonPhase() == MoonPhase.Empty)
						{
							Main.moonPhase = 5;
						}
						AchievementsHelper.NotifyProgressionEvent(4);
						NetMessage.TrySendData(7);
					}
				}
				else if (num238 == -11)
				{
					ChatHelper.BroadcastChatMessage(NetworkText.FromKey("Misc.CombatBookUsed"), ChatColors.World);
					NPC.combatBookWasUsed = true;
					NetMessage.TrySendData(7);
				}
				else if (num238 == -12)
				{
					NPC.UnlockOrExchangePet(ref NPC.boughtCat, 637, "Misc.LicenseCatUsed", num238);
				}
				else if (num238 == -13)
				{
					NPC.UnlockOrExchangePet(ref NPC.boughtDog, 638, "Misc.LicenseDogUsed", num238);
				}
				else if (num238 == -14)
				{
					NPC.UnlockOrExchangePet(ref NPC.boughtBunny, 656, "Misc.LicenseBunnyUsed", num238);
				}
				else if (num238 == -15)
				{
					NPC.UnlockOrExchangePet(ref NPC.unlockedSlimeBlueSpawn, 670, "Misc.LicenseSlimeUsed", num238);
				}
				else if (num238 == -16)
				{
					NPC.SpawnMechQueen(num237);
				}
				else if (num238 == -17)
				{
					ChatHelper.BroadcastChatMessage(NetworkText.FromKey("Misc.CombatBookVolumeTwoUsed"), ChatColors.World);
					NPC.combatBookVolumeTwoWasUsed = true;
					NetMessage.TrySendData(7);
				}
				else if (num238 == -18)
				{
					ChatHelper.BroadcastChatMessage(NetworkText.FromKey("Misc.PeddlersSatchelUsed"), ChatColors.World);
					NPC.peddlersSatchelWasUsed = true;
					NetMessage.TrySendData(7);
				}
				else if (num238 == -19)
				{
					Main.StartSlimeRain();
				}
				else if (num238 < 0)
				{
					int num239 = 1;
					if (num238 > -InvasionID.Count)
					{
						num239 = -num238;
					}
					if (num239 > 0 && Main.invasionType == 0)
					{
						Main.invasionDelay = 0;
						Main.StartInvasion(num239);
					}
					NetMessage.TrySendData(7);
					NetMessage.TrySendData(78, -1, -1, null, 0, 1f, Main.invasionType + 3);
				}
				break;
			}
			case 62:
			{
				int num173 = reader.ReadByte();
				int num174 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num173 = whoAmI;
				}
				if (num174 == 1)
				{
					Main.player[num173].NinjaDodge();
				}
				if (num174 == 2)
				{
					Main.player[num173].ShadowDodge();
				}
				if (num174 == 4)
				{
					Main.player[num173].BrainOfConfusionDodge();
				}
				if (num174 == 5)
				{
					Main.player[num173].DoMysticSashDodge();
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(62, -1, whoAmI, null, num173, num174);
				}
				break;
			}
			case 63:
			{
				int num145 = reader.ReadInt16();
				int num146 = reader.ReadInt16();
				byte b11 = reader.ReadByte();
				byte b12 = reader.ReadByte();
				if (b12 == 0)
				{
					WorldGen.paintTile(num145, num146, b11);
				}
				else
				{
					WorldGen.paintCoatTile(num145, num146, b11);
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(63, -1, whoAmI, null, num145, num146, (int)b11, (int)b12);
				}
				break;
			}
			case 64:
			{
				int num128 = reader.ReadInt16();
				int num129 = reader.ReadInt16();
				byte b9 = reader.ReadByte();
				byte b10 = reader.ReadByte();
				if (b10 == 0)
				{
					WorldGen.paintWall(num128, num129, b9);
				}
				else
				{
					WorldGen.paintCoatWall(num128, num129, b9);
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(64, -1, whoAmI, null, num128, num129, (int)b9, (int)b10);
				}
				break;
			}
			case 65:
			{
				BitsByte bitsByte18 = reader.ReadByte();
				int num82 = reader.ReadInt16();
				if (Main.netMode == 2)
				{
					num82 = whoAmI;
				}
				Vector2 vector2 = reader.ReadVector2();
				int num83 = 0;
				num83 = reader.ReadByte();
				int num84 = 0;
				if (bitsByte18[0])
				{
					num84++;
				}
				if (bitsByte18[1])
				{
					num84 += 2;
				}
				bool flag7 = false;
				if (bitsByte18[2])
				{
					flag7 = true;
				}
				int num85 = 0;
				if (bitsByte18[3])
				{
					num85 = reader.ReadInt32();
				}
				if (flag7)
				{
					vector2 = Main.player[num82].position;
				}
				switch (num84)
				{
				case 0:
					Main.player[num82].Teleport(vector2, num83, num85);
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(65, -1, whoAmI, null, 0, num82, vector2.X, vector2.Y, num83, flag7.ToInt(), num85);
					}
					if (Main.netMode == 1 && num82 == Main.myPlayer)
					{
						NetMessage.TrySendData(65, -1, -1, null, 3, num82);
					}
					break;
				case 1:
					Main.npc[num82].Teleport(vector2, num83, num85);
					Main.npc[num82].netOffset *= 0f;
					break;
				case 2:
				{
					Main.player[num82].Teleport(vector2, num83, num85);
					if (Main.netMode != 2)
					{
						break;
					}
					RemoteClient.CheckSection(whoAmI, vector2);
					NetMessage.TrySendData(65, -1, -1, null, 0, num82, vector2.X, vector2.Y, num83, flag7.ToInt(), num85);
					int num86 = -1;
					float num87 = 9999f;
					for (int num88 = 0; num88 < 255; num88++)
					{
						if (Main.player[num88].active && num88 != whoAmI)
						{
							Vector2 vector3 = Main.player[num88].position - Main.player[whoAmI].position;
							if (vector3.Length() < num87)
							{
								num87 = vector3.Length();
								num86 = num88;
							}
						}
					}
					if (num86 >= 0)
					{
						ChatHelper.BroadcastChatMessage(NetworkText.FromKey("Game.HasTeleportedTo", Main.player[whoAmI].name, Main.player[num86].name), new Color(250, 250, 0));
					}
					break;
				}
				case 3:
					Invariant.Assert(Main.netMode == 2, "TeleportEntity player ack on client");
					Invariant.Assert(Main.player[num82].unacknowledgedTeleports-- >= 0, "TeleportEntity player acks > teleports");
					break;
				}
				break;
			}
			case 66:
			{
				int num71 = reader.ReadByte();
				int num72 = reader.ReadInt16();
				if (num72 > 0)
				{
					Player player8 = Main.player[num71];
					player8.statLife += num72;
					if (player8.statLife > player8.statLifeMax2)
					{
						player8.statLife = player8.statLifeMax2;
					}
					player8.HealEffect(num72, broadcast: false);
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(66, -1, whoAmI, null, num71, num72);
					}
				}
				break;
			}
			case 68:
				reader.ReadString();
				break;
			case 69:
			{
				int num18 = reader.ReadInt16();
				int num19 = reader.ReadInt16();
				int num20 = reader.ReadInt16();
				if (Main.netMode == 1)
				{
					if (num18 >= 0 && num18 < 8000)
					{
						Chest chest = Main.chest[num18];
						if (chest == null)
						{
							chest = Chest.CreateWorldChest(num18, num19, num20);
						}
						else if (chest.x != num19 || chest.y != num20)
						{
							break;
						}
						chest.name = reader.ReadString();
					}
				}
				else
				{
					if (num18 < -1 || num18 >= 8000)
					{
						break;
					}
					if (num18 == -1)
					{
						num18 = Chest.FindChest(num19, num20);
						if (num18 == -1)
						{
							break;
						}
					}
					Chest chest2 = Main.chest[num18];
					if (chest2.x == num19 && chest2.y == num20)
					{
						NetMessage.TrySendData(69, whoAmI, -1, null, num18, num19, num20);
					}
				}
				break;
			}
			case 70:
				if (Main.netMode == 2)
				{
					int num16 = reader.ReadInt16();
					if (num16 >= 0 && num16 < Main.maxNPCs)
					{
						NPC.CatchNPC(num16, whoAmI);
					}
				}
				break;
			case 71:
				if (Main.netMode == 2)
				{
					int x15 = reader.ReadInt32();
					int y15 = reader.ReadInt32();
					int type20 = reader.ReadInt16();
					byte style2 = reader.ReadByte();
					NPC.ReleaseNPC(x15, y15, type20, style2, whoAmI);
				}
				break;
			case 72:
				if (Main.netMode == 1)
				{
					for (int num258 = 0; num258 < Main.TravelShopMaxSlots; num258++)
					{
						Main.travelShop[num258] = reader.ReadInt16();
					}
				}
				break;
			case 73:
				switch (reader.ReadByte())
				{
				case 0:
					Main.player[whoAmI].TeleportationPotion();
					break;
				case 1:
					Main.player[whoAmI].MagicConch();
					break;
				case 2:
					Main.player[whoAmI].DemonConch();
					break;
				case 3:
					Main.player[whoAmI].Shellphone_Spawn();
					break;
				case 4:
					Main.player[whoAmI].PlayerNoSpaceTeleport();
					break;
				}
				break;
			case 74:
				if (Main.netMode == 1)
				{
					Main.anglerQuest = reader.ReadByte();
					Main.anglerQuestFinished = reader.ReadBoolean();
				}
				break;
			case 75:
				if (Main.netMode == 2)
				{
					string name2 = Main.player[whoAmI].name;
					if (!Main.anglerWhoFinishedToday.Contains(name2))
					{
						Main.anglerWhoFinishedToday.Add(name2);
					}
				}
				break;
			case 76:
			{
				int num190 = reader.ReadByte();
				if (num190 != Main.myPlayer || Main.ServerSideCharacter)
				{
					if (Main.netMode == 2)
					{
						num190 = whoAmI;
					}
					Player obj8 = Main.player[num190];
					obj8.anglerQuestsFinished = reader.ReadInt32();
					obj8.golferScoreAccumulated = reader.ReadInt32();
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(76, -1, whoAmI, null, num190);
					}
				}
				break;
			}
			case 77:
			{
				short type18 = reader.ReadInt16();
				ushort tileType = reader.ReadUInt16();
				short x13 = reader.ReadInt16();
				short y13 = reader.ReadInt16();
				Animation.NewTemporaryAnimation(type18, tileType, x13, y13);
				break;
			}
			case 78:
				if (Main.netMode == 1)
				{
					Main.ReportInvasionProgress(reader.ReadInt32(), reader.ReadInt32(), reader.ReadSByte(), reader.ReadSByte());
				}
				break;
			case 79:
			{
				int x12 = reader.ReadInt16();
				int y12 = reader.ReadInt16();
				short type17 = reader.ReadInt16();
				int style = reader.ReadInt16();
				int num165 = reader.ReadByte();
				int random = reader.ReadSByte();
				int direction = (reader.ReadBoolean() ? 1 : (-1));
				if (Main.netMode == 2)
				{
					Netplay.Clients[whoAmI].SpamAddBlock += 1f;
					if (!WorldGen.InWorld(x12, y12, 10) || !Netplay.Clients[whoAmI].TileSections[Netplay.GetSectionX(x12), Netplay.GetSectionY(y12)])
					{
						break;
					}
				}
				WorldGen.PlaceObject(x12, y12, type17, mute: false, style, num165, random, direction);
				if (Main.netMode == 2)
				{
					NetMessage.SendObjectPlacement(whoAmI, x12, y12, type17, style, num165, random, direction);
				}
				break;
			}
			case 80:
				if (Main.netMode == 1)
				{
					int num151 = reader.ReadByte();
					int num152 = reader.ReadInt16();
					if (num152 >= -3 && num152 < 8000)
					{
						Main.player[num151].chest = num152;
					}
				}
				break;
			case 81:
				if (Main.netMode == 1)
				{
					int x10 = (int)reader.ReadSingle();
					int y10 = (int)reader.ReadSingle();
					CombatText.NewText(color: reader.ReadRGB(), amount: reader.ReadInt32(), location: new Rectangle(x10, y10, 0, 0));
				}
				break;
			case 119:
				if (Main.netMode == 1)
				{
					int x11 = (int)reader.ReadSingle();
					int y11 = (int)reader.ReadSingle();
					CombatText.NewText(color: reader.ReadRGB(), text: NetworkText.Deserialize(reader).ToString(), location: new Rectangle(x11, y11, 0, 0));
				}
				break;
			case 82:
				NetManager.Instance.Read(reader, whoAmI, length);
				break;
			case 84:
			{
				int num130 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num130 = whoAmI;
				}
				float stealth = reader.ReadSingle();
				Main.player[num130].stealth = stealth;
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(84, -1, whoAmI, null, num130);
				}
				break;
			}
			case 85:
				if (Main.netMode == 2 && whoAmI < 255)
				{
					Player player15 = Main.player[whoAmI];
					QuickStacking.SourceInventory inventory = QuickStacking.ReadNetInventory(player15, reader);
					bool smartStack = reader.ReadBoolean();
					QuickStacking.QuickStackToNearbyChests(player15, inventory, smartStack);
				}
				else if (Main.netMode == 1)
				{
					QuickStacking.IndicateBlockedChests(Main.LocalPlayer, QuickStacking.ReadBlockedChestList(reader));
				}
				break;
			case 86:
			{
				if (Main.netMode != 1)
				{
					break;
				}
				int num116 = reader.ReadInt32();
				if (!reader.ReadBoolean())
				{
					if (TileEntity.TryGet<TileEntity>(num116, out var result3))
					{
						TileEntity.Remove(result3);
					}
				}
				else
				{
					TileEntity tileEntity = TileEntity.Read(reader, 326, networkSend: true);
					tileEntity.ID = num116;
					TileEntity.Add(tileEntity);
				}
				break;
			}
			case 87:
				if (Main.netMode == 2)
				{
					int x8 = reader.ReadInt16();
					int y8 = reader.ReadInt16();
					int type10 = reader.ReadByte();
					if (WorldGen.InWorld(x8, y8) && !TileEntity.TryGetAt<TileEntity>(x8, y8, out var _))
					{
						TileEntity.PlaceEntityNet(x8, y8, type10);
					}
				}
				break;
			case 88:
			{
				if (Main.netMode != 1)
				{
					break;
				}
				int num2 = reader.ReadInt16();
				if (num2 < 0 || num2 > 400)
				{
					break;
				}
				Item inner = Main.item[num2].inner;
				BitsByte bitsByte = reader.ReadByte();
				if (bitsByte[0])
				{
					inner.color.PackedValue = reader.ReadUInt32();
				}
				if (bitsByte[1])
				{
					inner.damage = reader.ReadUInt16();
				}
				if (bitsByte[2])
				{
					inner.knockBack = reader.ReadSingle();
				}
				if (bitsByte[3])
				{
					inner.useAnimation = reader.ReadUInt16();
				}
				if (bitsByte[4])
				{
					inner.useTime = reader.ReadUInt16();
				}
				if (bitsByte[5])
				{
					inner.shoot = reader.ReadInt16();
				}
				if (bitsByte[6])
				{
					inner.shootSpeed = reader.ReadSingle();
				}
				if (bitsByte[7])
				{
					bitsByte = reader.ReadByte();
					if (bitsByte[0])
					{
						inner.width = reader.ReadInt16();
					}
					if (bitsByte[1])
					{
						inner.height = reader.ReadInt16();
					}
					if (bitsByte[2])
					{
						inner.scale = reader.ReadSingle();
					}
					if (bitsByte[3])
					{
						inner.ammo = reader.ReadInt16();
					}
					if (bitsByte[4])
					{
						inner.useAmmo = reader.ReadInt16();
					}
					if (bitsByte[5])
					{
						inner.notAmmo = reader.ReadBoolean();
					}
				}
				break;
			}
			case 89:
			{
				if (Main.netMode != 2)
				{
					break;
				}
				int x = reader.ReadInt16();
				int y = reader.ReadInt16();
				int type = reader.ReadInt16();
				int prefix = reader.ReadByte();
				int stack = reader.ReadInt16();
				using (Item.DefaultAssignNewItemsToPlayer(whoAmI))
				{
					TEItemFrame.TryPlacing(x, y, type, prefix, stack);
					break;
				}
			}
			case 91:
			{
				if (Main.netMode != 1)
				{
					break;
				}
				int num246 = reader.ReadInt32();
				int num247 = reader.ReadByte();
				if (num247 == 255)
				{
					if (EmoteBubble.byID.ContainsKey(num246))
					{
						EmoteBubble.byID.Remove(num246);
					}
					break;
				}
				int num248 = reader.ReadUInt16();
				int num249 = reader.ReadUInt16();
				int num250 = reader.ReadByte();
				int metadata = 0;
				if (num250 < 0)
				{
					metadata = reader.ReadInt16();
				}
				WorldUIAnchor worldUIAnchor = EmoteBubble.DeserializeNetAnchor(num247, num248);
				if (num247 == 1)
				{
					Main.player[num248].emoteTime = 360;
				}
				lock (EmoteBubble.byID)
				{
					if (!EmoteBubble.byID.ContainsKey(num246))
					{
						EmoteBubble.byID[num246] = new EmoteBubble(num250, worldUIAnchor, num249);
					}
					else
					{
						EmoteBubble.byID[num246].lifeTime = num249;
						EmoteBubble.byID[num246].lifeTimeStart = num249;
						EmoteBubble.byID[num246].emote = num250;
						EmoteBubble.byID[num246].anchor = worldUIAnchor;
					}
					EmoteBubble.byID[num246].ID = num246;
					EmoteBubble.byID[num246].metadata = metadata;
					EmoteBubble.OnBubbleChange(num246);
					break;
				}
			}
			case 92:
			{
				int num231 = reader.ReadInt16();
				int num232 = reader.ReadInt32();
				float num233 = reader.ReadSingle();
				float num234 = reader.ReadSingle();
				if (num231 >= 0 && num231 <= Main.maxNPCs)
				{
					if (Main.netMode == 1)
					{
						Main.npc[num231].moneyPing(new Vector2(num233, num234));
						Main.npc[num231].extraValue = num232;
					}
					else
					{
						Main.npc[num231].extraValue += num232;
						NetMessage.TrySendData(92, -1, -1, null, num231, Main.npc[num231].extraValue, num233, num234);
					}
				}
				break;
			}
			case 94:
			{
				string text5 = reader.ReadString();
				int num226 = reader.ReadInt32();
				int num227 = (int)reader.ReadSingle();
				reader.ReadSingle();
				if (!DebugOptions.enableDebugCommands)
				{
					break;
				}
				switch (text5)
				{
				case "/showdebug":
					DebugOptions.Shared_ReportCommandUsage = num227 == 1;
					break;
				case "/randomizeprojslots":
					DebugOptions.Shared_RandomizeProjectileSlots = num227 == 1;
					break;
				case "/setserverping":
					DebugOptions.Shared_ServerPing = num227;
					DebugNetworkStream.Latency = (uint)(num227 / 2);
					break;
				case "/quickload-clientprobe":
					if (Main.netMode == 2)
					{
						NetMessage.SendData(94, -1, whoAmI, NetworkText.FromLiteral(text5), whoAmI);
					}
					else if (Netplay.ServerIPText == "127.0.0.1")
					{
						NetMessage.SendData(94, -1, -1, NetworkText.FromLiteral("/quickload-clientresp " + QuickLoad.Serialize(new QuickLoad.JoinWorld().WithCurrentState())), num226);
					}
					break;
				default:
				{
					if (!text5.StartsWith("/quickload-clientresp "))
					{
						break;
					}
					if (Main.netMode == 2)
					{
						NetMessage.SendData(94, num226, -1, NetworkText.FromLiteral(text5));
						break;
					}
					QuickLoad.JoinWorld joinWorld = (QuickLoad.JoinWorld)QuickLoad.Deserialize(text5.Substring("/quickload-clientresp ".Length));
					if (QuickLoad.TryRead(out var config) && config is QuickLoad.JoinWorld)
					{
						QuickLoad.JoinWorld obj9 = (QuickLoad.JoinWorld)config;
						obj9.ExtraClients.Add(joinWorld);
						QuickLoad.Set(obj9);
						ChatHelper.DisplayMessage(NetworkText.FromLiteral("/quickload added " + Path.GetFileName(joinWorld.PlayerPath)), new Color(250, 250, 0), byte.MaxValue);
					}
					break;
				}
				}
				break;
			}
			case 95:
			{
				ushort num191 = reader.ReadUInt16();
				int num192 = reader.ReadByte();
				if (Main.netMode != 2)
				{
					break;
				}
				for (int num193 = 0; num193 < 1000; num193++)
				{
					if (Main.projectile[num193].owner == num191 && Main.projectile[num193].active && Main.projectile[num193].type == 602 && Main.projectile[num193].ai[1] == (float)num192)
					{
						Main.projectile[num193].Kill();
						NetMessage.TrySendData(29, -1, -1, null, Main.projectile[num193].key);
						break;
					}
				}
				break;
			}
			case 96:
			{
				int num186 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num186 = whoAmI;
				}
				Player obj7 = Main.player[num186];
				int num187 = reader.ReadInt16();
				Vector2 newPos2 = reader.ReadVector2();
				Vector2 velocity7 = reader.ReadVector2();
				int lastPortalColorIndex2 = num187 + ((num187 % 2 == 0) ? 1 : (-1));
				obj7.lastPortalColorIndex = lastPortalColorIndex2;
				obj7.Teleport(newPos2, 4, num187);
				obj7.velocity = velocity7;
				if (Main.netMode == 2)
				{
					NetMessage.SendData(96, -1, num186, null, num186, newPos2.X, newPos2.Y, num187);
				}
				break;
			}
			case 97:
				if (Main.netMode == 1)
				{
					AchievementsHelper.NotifyNPCKilledDirect(Main.player[Main.myPlayer], reader.ReadInt16());
				}
				break;
			case 98:
				if (Main.netMode == 1)
				{
					AchievementsHelper.NotifyProgressionEvent(reader.ReadInt16());
				}
				break;
			case 99:
			{
				int num166 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num166 = whoAmI;
				}
				Main.player[num166].MinionRestTargetPoint = reader.ReadVector2();
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(99, -1, whoAmI, null, num166);
				}
				break;
			}
			case 115:
			{
				int num157 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num157 = whoAmI;
				}
				Main.player[num157].MinionAttackTargetNPC = reader.ReadInt16();
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(115, -1, whoAmI, null, num157);
				}
				break;
			}
			case 100:
			{
				int num147 = reader.ReadUInt16();
				NPC obj5 = Main.npc[num147];
				int num148 = reader.ReadInt16();
				Vector2 newPos = reader.ReadVector2();
				Vector2 velocity4 = reader.ReadVector2();
				int lastPortalColorIndex = num148 + ((num148 % 2 == 0) ? 1 : (-1));
				obj5.lastPortalColorIndex = lastPortalColorIndex;
				obj5.Teleport(newPos, 4, num148);
				obj5.velocity = velocity4;
				obj5.netOffset *= 0f;
				break;
			}
			case 101:
				if (Main.netMode != 2)
				{
					NPC.ShieldStrengthTowerSolar = reader.ReadUInt16();
					NPC.ShieldStrengthTowerVortex = reader.ReadUInt16();
					NPC.ShieldStrengthTowerNebula = reader.ReadUInt16();
					NPC.ShieldStrengthTowerStardust = reader.ReadUInt16();
					if (NPC.ShieldStrengthTowerSolar < 0)
					{
						NPC.ShieldStrengthTowerSolar = 0;
					}
					if (NPC.ShieldStrengthTowerVortex < 0)
					{
						NPC.ShieldStrengthTowerVortex = 0;
					}
					if (NPC.ShieldStrengthTowerNebula < 0)
					{
						NPC.ShieldStrengthTowerNebula = 0;
					}
					if (NPC.ShieldStrengthTowerStardust < 0)
					{
						NPC.ShieldStrengthTowerStardust = 0;
					}
					if (NPC.ShieldStrengthTowerSolar > NPC.LunarShieldPowerMax)
					{
						NPC.ShieldStrengthTowerSolar = NPC.LunarShieldPowerMax;
					}
					if (NPC.ShieldStrengthTowerVortex > NPC.LunarShieldPowerMax)
					{
						NPC.ShieldStrengthTowerVortex = NPC.LunarShieldPowerMax;
					}
					if (NPC.ShieldStrengthTowerNebula > NPC.LunarShieldPowerMax)
					{
						NPC.ShieldStrengthTowerNebula = NPC.LunarShieldPowerMax;
					}
					if (NPC.ShieldStrengthTowerStardust > NPC.LunarShieldPowerMax)
					{
						NPC.ShieldStrengthTowerStardust = NPC.LunarShieldPowerMax;
					}
				}
				break;
			case 102:
			{
				int num107 = reader.ReadByte();
				ushort num108 = reader.ReadUInt16();
				Vector2 other = reader.ReadVector2();
				if (Main.netMode == 2)
				{
					num107 = whoAmI;
					NetMessage.TrySendData(102, -1, -1, null, num107, (int)num108, other.X, other.Y);
					break;
				}
				Player player11 = Main.player[num107];
				for (int num109 = 0; num109 < 255; num109++)
				{
					Player player12 = Main.player[num109];
					if (!player12.active || player12.dead || (player11.team != 0 && player11.team != player12.team) || !(player12.Distance(other) < 700f))
					{
						continue;
					}
					Vector2 value2 = player11.Center - player12.Center;
					Vector2 vector4 = Vector2.Normalize(value2);
					if (!vector4.HasNaNs())
					{
						int type11 = 90;
						float num110 = 0f;
						float num111 = (float)Math.PI / 15f;
						Vector2 spinningpoint = new Vector2(0f, -8f);
						Vector2 vector5 = new Vector2(-3f);
						float num112 = 0f;
						float num113 = 0.005f;
						switch (num108)
						{
						case 179:
							type11 = 86;
							break;
						case 173:
							type11 = 90;
							break;
						case 176:
							type11 = 88;
							break;
						}
						for (int num114 = 0; (float)num114 < value2.Length() / 6f; num114++)
						{
							Vector2 position2 = player12.Center + 6f * (float)num114 * vector4 + spinningpoint.RotatedBy(num110) + vector5;
							num110 += num111;
							int num115 = Dust.NewDust(position2, 6, 6, type11, 0f, 0f, 100, default(Color), 1.5f);
							Main.dust[num115].noGravity = true;
							Main.dust[num115].velocity = Vector2.Zero;
							num112 = (Main.dust[num115].fadeIn = num112 + num113);
							Main.dust[num115].velocity += vector4 * 1.5f;
						}
					}
					player12.NebulaLevelup(num108);
				}
				break;
			}
			case 103:
				if (Main.netMode == 1)
				{
					NPC.MaxMoonLordCountdown = reader.ReadInt32();
					NPC.MoonLordCountdown = reader.ReadInt32();
				}
				break;
			case 104:
				if (Main.netMode == 1 && Main.npcShop > 0)
				{
					Item[] item4 = Main.instance.shop[Main.npcShop].item;
					int num101 = reader.ReadByte();
					int type9 = reader.ReadInt16();
					int stack5 = reader.ReadInt16();
					int prefixWeWant2 = reader.ReadByte();
					int value = reader.ReadInt32();
					BitsByte bitsByte22 = reader.ReadByte();
					if (num101 < item4.Length)
					{
						item4[num101] = new Item();
						item4[num101].SetDefaults(type9);
						item4[num101].stack = stack5;
						item4[num101].Prefix(prefixWeWant2);
						item4[num101].value = value;
						item4[num101].buyOnce = bitsByte22[0];
					}
				}
				break;
			case 105:
				if (Main.netMode != 1)
				{
					short i2 = reader.ReadInt16();
					int j2 = reader.ReadInt16();
					bool flag11 = reader.ReadBoolean();
					WorldGen.ToggleGemLock(i2, j2, flag11);
				}
				break;
			case 106:
				if (Main.netMode == 1)
				{
					HalfVector2 halfVector = new HalfVector2
					{
						PackedValue = reader.ReadUInt32()
					};
					Utils.PoofOfSmoke(halfVector.ToVector2());
				}
				break;
			case 107:
				if (Main.netMode == 1)
				{
					Color c = reader.ReadRGB();
					string text4 = NetworkText.Deserialize(reader).ToString();
					int widthLimit = reader.ReadInt16();
					Main.NewTextMultiline(text4, force: false, c, widthLimit);
				}
				break;
			case 108:
				if (Main.netMode == 1)
				{
					int damage2 = reader.ReadInt16();
					float knockBack = reader.ReadSingle();
					int x7 = reader.ReadInt16();
					int y7 = reader.ReadInt16();
					int angle = reader.ReadInt16();
					int ammo = reader.ReadInt16();
					int num77 = reader.ReadByte();
					if (num77 == Main.myPlayer)
					{
						WorldGen.ShootFromCannon(x7, y7, angle, ammo, damage2, knockBack, num77, fromWire: true);
					}
				}
				break;
			case 109:
				if (Main.netMode == 2)
				{
					short x5 = reader.ReadInt16();
					int y5 = reader.ReadInt16();
					int x6 = reader.ReadInt16();
					int y6 = reader.ReadInt16();
					byte toolMode = reader.ReadByte();
					int num74 = whoAmI;
					WiresUI.Settings.MultiToolMode toolMode2 = WiresUI.Settings.ToolMode;
					WiresUI.Settings.ToolMode = (WiresUI.Settings.MultiToolMode)toolMode;
					Wiring.MassWireOperation(new Point(x5, y5), new Point(x6, y6), Main.player[num74]);
					WiresUI.Settings.ToolMode = toolMode2;
				}
				break;
			case 110:
			{
				if (Main.netMode != 1)
				{
					break;
				}
				int type5 = reader.ReadInt16();
				int num67 = reader.ReadInt16();
				int num68 = reader.ReadByte();
				if (num68 == Main.myPlayer)
				{
					Player player7 = Main.player[num68];
					for (int k = 0; k < num67; k++)
					{
						player7.ConsumeItem(type5);
					}
					player7.wireOperationsCooldown = 0;
				}
				break;
			}
			case 111:
				if (Main.netMode == 2)
				{
					BirthdayParty.ToggleManualParty();
				}
				break;
			case 112:
			{
				int num62 = reader.ReadByte();
				int num63 = reader.ReadInt32();
				int num64 = reader.ReadInt32();
				int num65 = reader.ReadByte();
				int num66 = reader.ReadInt16();
				bool flag6 = reader.ReadByte() == 1;
				switch (num62)
				{
				case 1:
					if (Main.netMode == 1)
					{
						WorldGen.TreeGrowFX(num63, num64, num65, num66, flag6);
					}
					if (Main.netMode == 2)
					{
						NetMessage.TrySendData(b, -1, -1, null, num62, num63, num64, num65, num66, flag6 ? 1 : 0);
					}
					break;
				case 2:
					NPC.FairyEffects(new Vector2(num63, num64), num66);
					break;
				}
				break;
			}
			case 113:
			{
				int x2 = reader.ReadInt16();
				int y2 = reader.ReadInt16();
				if (Main.netMode == 2 && !Main.snowMoon && !Main.pumpkinMoon)
				{
					if (DD2Event.WouldFailSpawningHere(x2, y2))
					{
						DD2Event.FailureMessage(whoAmI);
					}
					DD2Event.SummonCrystal(x2, y2, whoAmI);
				}
				break;
			}
			case 114:
				if (Main.netMode == 1)
				{
					DD2Event.WipeEntities();
				}
				break;
			case 116:
				if (Main.netMode == 1)
				{
					DD2Event.TimeLeftBetweenWaves = reader.ReadInt32();
				}
				break;
			case 117:
			{
				int num27 = reader.ReadByte();
				if (Main.netMode != 2 || whoAmI == num27 || (Main.player[num27].hostile && Main.player[whoAmI].hostile))
				{
					PlayerDeathReason playerDeathReason2 = PlayerDeathReason.FromReader(reader);
					int damage = reader.ReadInt16();
					int num28 = reader.ReadByte() - 1;
					BitsByte bitsByte3 = reader.ReadByte();
					bool flag2 = bitsByte3[0];
					bool pvp2 = bitsByte3[1];
					int num29 = reader.ReadSByte();
					Main.player[num27].Hurt(playerDeathReason2, damage, num28, pvp2, quiet: true, flag2, num29);
					if (Main.netMode == 2)
					{
						NetMessage.SendPlayerHurt(num27, playerDeathReason2, damage, num28, flag2, pvp2, num29, -1, whoAmI);
					}
				}
				break;
			}
			case 118:
			{
				int num13 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num13 = whoAmI;
				}
				PlayerDeathReason playerDeathReason = PlayerDeathReason.FromReader(reader);
				int num14 = reader.ReadInt16();
				int num15 = reader.ReadByte() - 1;
				bool pvp = ((BitsByte)reader.ReadByte())[0];
				Main.player[num13].KillMe(playerDeathReason, num14, num15, pvp);
				if (Main.netMode == 2)
				{
					NetMessage.SendPlayerDeath(num13, playerDeathReason, num14, num15, pvp, -1, whoAmI);
				}
				break;
			}
			case 120:
			{
				int num259 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num259 = whoAmI;
				}
				int num260 = reader.ReadByte();
				if (num260 >= 0 && num260 < EmoteID.Count && Main.netMode == 2)
				{
					EmoteBubble.NewBubble(num260, new WorldUIAnchor(Main.player[num259]), 360);
					EmoteBubble.CheckForNPCsToReactToEmoteBubble(num260, Main.player[num259]);
				}
				break;
			}
			case 121:
			{
				int num251 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num251 = whoAmI;
				}
				int num252 = reader.ReadInt32();
				int num253 = reader.ReadByte();
				int num254 = reader.ReadByte();
				if (!TileEntity.TryGet<TEDisplayDoll>(num252, out var result7))
				{
					TEDisplayDoll.ReadDummySync(num253, num254, reader);
					break;
				}
				result7.ReadData(num253, num254, reader);
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(b, -1, num251, null, num251, num252, num253, num254);
				}
				break;
			}
			case 122:
			{
				int num224 = reader.ReadInt32();
				int num225 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num225 = whoAmI;
				}
				if (Main.netMode == 2)
				{
					if (num224 == -1)
					{
						Main.player[num225].tileEntityAnchor.Clear();
						NetMessage.TrySendData(b, -1, -1, null, num224, num225);
						break;
					}
					if (!TileEntity.IsOccupied(num224, out var _) && TileEntity.TryGet<TileEntity>(num224, out var result5))
					{
						Main.player[num225].tileEntityAnchor.Set(num224, result5.Position.X, result5.Position.Y);
						NetMessage.TrySendData(b, -1, -1, null, num224, num225);
					}
				}
				if (Main.netMode == 1)
				{
					TileEntity result6;
					if (num224 == -1)
					{
						Main.player[num225].tileEntityAnchor.Clear();
					}
					else if (TileEntity.TryGet<TileEntity>(num224, out result6))
					{
						TileEntity.SetInteractionAnchor(Main.player[num225], result6.Position.X, result6.Position.Y, num224);
					}
				}
				break;
			}
			case 123:
			{
				if (Main.netMode != 2)
				{
					break;
				}
				int x14 = reader.ReadInt16();
				int y14 = reader.ReadInt16();
				int type19 = reader.ReadInt16();
				int prefix4 = reader.ReadByte();
				int stack8 = reader.ReadInt16();
				using (Item.DefaultAssignNewItemsToPlayer(whoAmI))
				{
					TEWeaponsRack.TryPlacing(x14, y14, type19, prefix4, stack8);
					break;
				}
			}
			case 124:
			{
				int num183 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num183 = whoAmI;
				}
				int num184 = reader.ReadInt32();
				int num185 = reader.ReadByte();
				bool flag18 = false;
				if (num185 >= 2)
				{
					flag18 = true;
					num185 -= 2;
				}
				if (!TileEntity.TryGet<TEHatRack>(num184, out var result4) || num185 >= 2)
				{
					reader.ReadInt32();
					reader.ReadByte();
					break;
				}
				result4.ReadItem(num185, reader, flag18);
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(b, -1, num183, null, num183, num184, num185, flag18.ToInt());
				}
				break;
			}
			case 125:
			{
				int num169 = reader.ReadByte();
				int num170 = reader.ReadInt16();
				int num171 = reader.ReadInt16();
				int num172 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num169 = whoAmI;
				}
				if (Main.netMode == 1)
				{
					Main.player[Main.myPlayer].GetOtherPlayersPickTile(num170, num171, num172);
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(125, -1, num169, null, num169, num170, num171, num172);
				}
				break;
			}
			case 126:
				if (Main.netMode == 1)
				{
					NPC.RevengeManager.AddMarkerFromReader(reader);
				}
				break;
			case 127:
			{
				int markerUniqueID = reader.ReadInt32();
				if (Main.netMode == 1)
				{
					NPC.RevengeManager.DestroyMarker(markerUniqueID);
				}
				break;
			}
			case 128:
			{
				int num160 = reader.ReadByte();
				int num161 = reader.ReadUInt16();
				int num162 = reader.ReadUInt16();
				int num163 = reader.ReadUInt16();
				int num164 = reader.ReadUInt16();
				if (Main.netMode == 2)
				{
					NetMessage.SendData(128, -1, num160, null, num160, num163, num164, 0f, num161, num162);
				}
				else
				{
					GolfHelper.ContactListener.PutBallInCup_TextAndEffects(new Point(num161, num162), num160, num163, num164);
				}
				break;
			}
			case 129:
				if (Main.netMode == 1)
				{
					if (Main.LocalPlayer.team > 0)
					{
						NetMessage.SendData(45, -1, -1, null, Main.myPlayer);
					}
					Main.FixUIScale();
					Main.TrySetPreparationState(Main.WorldPreparationState.ProcessingData);
				}
				break;
			case 130:
			{
				if (Main.netMode != 2)
				{
					break;
				}
				int num141 = reader.ReadUInt16();
				int num142 = reader.ReadUInt16();
				int num143 = reader.ReadInt16();
				if (num143 == 682)
				{
					if (NPC.unlockedSlimeRedSpawn)
					{
						break;
					}
					NPC.unlockedSlimeRedSpawn = true;
					NetMessage.TrySendData(7);
				}
				num141 *= 16;
				num142 *= 16;
				NPC nPC4 = new NPC();
				nPC4.SetDefaults(num143);
				int type14 = nPC4.type;
				int netID = nPC4.netID;
				int num144 = NPC.NewNPC(new EntitySource_FishedOut(Main.player[whoAmI]), num141, num142, num143);
				if (netID != type14)
				{
					Main.npc[num144].SetDefaults(netID);
					NetMessage.TrySendData(23, -1, -1, null, num144);
				}
				if (num143 == 682)
				{
					WorldGen.CheckAchievement_RealEstateAndTownSlimes();
				}
				break;
			}
			case 131:
				if (Main.netMode == 1)
				{
					int num132 = reader.ReadUInt16();
					NPC nPC3 = null;
					nPC3 = ((num132 >= Main.maxNPCs) ? new NPC() : Main.npc[num132]);
					int num133 = reader.ReadByte();
					if (num133 == 1)
					{
						int time = reader.ReadInt32();
						int fromWho = reader.ReadInt16();
						nPC3.GetImmuneTime(fromWho, time);
					}
				}
				break;
			case 132:
				if (Main.netMode == 1)
				{
					Point point2 = reader.ReadVector2().ToPoint();
					ushort key = reader.ReadUInt16();
					LegacySoundStyle legacySoundStyle = SoundID.SoundByIndex[key];
					BitsByte bitsByte23 = reader.ReadByte();
					int num117 = -1;
					float num118 = 1f;
					float num119 = 0f;
					SoundEngine.PlaySound(Style: (!bitsByte23[0]) ? legacySoundStyle.Style : reader.ReadInt32(), volumeScale: (!bitsByte23[1]) ? legacySoundStyle.Volume : MathHelper.Clamp(reader.ReadSingle(), 0f, 1f), pitchOffset: (!bitsByte23[2]) ? legacySoundStyle.GetRandomPitch() : MathHelper.Clamp(reader.ReadSingle(), -1f, 1f), type: legacySoundStyle.SoundId, x: point2.X, y: point2.Y);
				}
				break;
			case 133:
			{
				if (Main.netMode != 2)
				{
					break;
				}
				int x9 = reader.ReadInt16();
				int y9 = reader.ReadInt16();
				int type12 = reader.ReadInt16();
				int prefix3 = reader.ReadByte();
				int stack6 = reader.ReadInt16();
				using (Item.DefaultAssignNewItemsToPlayer(whoAmI))
				{
					TEFoodPlatter.TryPlacing(x9, y9, type12, prefix3, stack6);
					break;
				}
			}
			case 134:
			{
				int num106 = reader.ReadByte();
				int ladyBugLuckTimeLeft = reader.ReadInt32();
				float torchLuck = reader.ReadSingle();
				byte luckPotion = reader.ReadByte();
				bool hasGardenGnomeNearby = reader.ReadBoolean();
				bool brokenMirrorBadLuck = reader.ReadBoolean();
				float equipmentBasedLuckBonus = reader.ReadSingle();
				float coinLuck = reader.ReadSingle();
				byte kiteLuckLevel = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num106 = whoAmI;
				}
				Player obj4 = Main.player[num106];
				obj4.ladyBugLuckTimeLeft = ladyBugLuckTimeLeft;
				obj4.torchLuck = torchLuck;
				obj4.luckPotion = luckPotion;
				obj4.HasGardenGnomeNearby = hasGardenGnomeNearby;
				obj4.brokenMirrorBadLuck = brokenMirrorBadLuck;
				obj4.equipmentBasedLuckBonus = equipmentBasedLuckBonus;
				obj4.coinLuck = coinLuck;
				obj4.kiteLuckLevel = kiteLuckLevel;
				obj4.RecalculateLuck();
				if (Main.netMode == 2)
				{
					NetMessage.SendData(134, -1, num106, null, num106);
				}
				break;
			}
			case 135:
			{
				int num105 = reader.ReadByte();
				if (Main.netMode == 1)
				{
					Main.player[num105].immuneAlpha = 255;
				}
				break;
			}
			case 136:
			{
				if (Main.netMode == 2)
				{
					break;
				}
				for (int num102 = 0; num102 < 2; num102++)
				{
					for (int num103 = 0; num103 < 3; num103++)
					{
						NPC.cavernMonsterType[num102, num103] = reader.ReadUInt16();
					}
				}
				break;
			}
			case 137:
				if (Main.netMode == 2)
				{
					int num100 = reader.ReadInt16();
					int buffTypeToRemove = reader.ReadUInt16();
					if (num100 >= 0 && num100 < Main.maxNPCs)
					{
						Main.npc[num100].RequestBuffRemoval(buffTypeToRemove);
					}
				}
				break;
			case 139:
				if (Main.netMode != 2)
				{
					int num99 = reader.ReadByte();
					bool flag10 = reader.ReadBoolean();
					Main.countsAsHostForGameplay[num99] = flag10;
				}
				break;
			case 140:
			{
				int num97 = reader.ReadByte();
				int num98 = reader.ReadInt32();
				switch (num97)
				{
				case 0:
					if (Main.netMode == 1)
					{
						CreditsRollEvent.SetRemainingTimeDirect(num98);
					}
					break;
				case 1:
					if (Main.netMode == 2)
					{
						NPC.TransformCopperSlime(num98);
					}
					break;
				case 2:
					if (Main.netMode == 2)
					{
						NPC.TransformElderSlime(num98);
					}
					break;
				}
				break;
			}
			case 141:
			{
				LucyAxeMessage.MessageSource messageSource = (LucyAxeMessage.MessageSource)reader.ReadByte();
				byte b6 = reader.ReadByte();
				Vector2 velocity2 = reader.ReadVector2();
				int num89 = reader.ReadInt32();
				int num90 = reader.ReadInt32();
				if (Main.netMode == 2)
				{
					NetMessage.SendData(141, -1, whoAmI, null, (int)messageSource, (int)b6, velocity2.X, velocity2.Y, num89, num90);
				}
				else
				{
					LucyAxeMessage.CreateFromNet(messageSource, b6, new Vector2(num89, num90), velocity2);
				}
				break;
			}
			case 142:
			{
				int num81 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num81 = whoAmI;
				}
				Player obj3 = Main.player[num81];
				obj3.piggyBankProjTracker.Read(reader);
				obj3.voidLensChest.Read(reader);
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(142, -1, whoAmI, null, num81);
				}
				break;
			}
			case 143:
				if (Main.netMode == 2)
				{
					DD2Event.AttemptToSkipWaitTime();
				}
				break;
			case 144:
				if (Main.netMode == 2)
				{
					NPC.HaveDryadDoStardewAnimation();
				}
				break;
			case 146:
				if (Main.netMode == 1)
				{
					switch (reader.ReadByte())
					{
					case 0:
						WorldItem.ShimmerEffect(reader.ReadVector2());
						break;
					case 1:
					{
						Vector2 coinPosition = reader.ReadVector2();
						int coinAmount = reader.ReadInt32();
						Main.player[Main.myPlayer].AddCoinLuck(coinPosition, coinAmount);
						break;
					}
					}
				}
				break;
			case 147:
			{
				int num69 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num69 = whoAmI;
				}
				int num70 = reader.ReadByte();
				Main.player[num69].TrySwitchingLoadout(num70);
				ReadAccessoryVisibility(reader, Main.player[num69].hideVisibleAccessory);
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(b, -1, num69, null, num69, num70);
				}
				break;
			}
			case 149:
			{
				if (Main.netMode != 2)
				{
					break;
				}
				int x4 = reader.ReadInt16();
				int y4 = reader.ReadInt16();
				int type6 = reader.ReadInt16();
				int prefix2 = reader.ReadByte();
				int stack4 = reader.ReadInt16();
				using (Item.DefaultAssignNewItemsToPlayer(whoAmI))
				{
					TEDeadCellsDisplayJar.TryPlacing(x4, y4, type6, prefix2, stack4);
					break;
				}
			}
			case 150:
			{
				int num57 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num57 = whoAmI;
				}
				int num58 = reader.ReadInt16();
				Player player6 = Main.player[num57];
				if (Main.netMode == 2)
				{
					if (num58 >= 0)
					{
						player6.SetOrRequestSpectating(num58);
						break;
					}
					player6.spectating = -1;
					NetMessage.SendData(150, -1, whoAmI, null, whoAmI, num58);
				}
				else if (player6 != Main.LocalPlayer || player6.spectating >= 0)
				{
					player6.spectating = num58;
				}
				break;
			}
			case 152:
			{
				int num51 = reader.ReadByte();
				if (Main.netMode == 2)
				{
					num51 = whoAmI;
				}
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(152, -1, whoAmI, null, num51);
				}
				if (Main.netMode == 1)
				{
					Player player5 = Main.player[num51];
					Item item3 = player5.inventory[player5.selectedItem];
					if (item3.UseSound != null)
					{
						SoundEngine.PlaySound(item3.UseSound, player5.Center, item3.useSoundPitch);
					}
				}
				break;
			}
			case 153:
			{
				int num49 = reader.ReadByte();
				int num50 = reader.ReadInt16();
				Main.npc[num49].GetHurtByDebuff(num50);
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(153, -1, whoAmI, null, num49, num50);
				}
				break;
			}
			case 154:
				if (Main.netMode == 2)
				{
					NetMessage.TrySendData(154, whoAmI);
				}
				else
				{
					Ping.PingRecieved();
				}
				break;
			case 155:
			{
				short num31 = reader.ReadInt16();
				short newSize = reader.ReadInt16();
				if (num31 >= 0 && num31 < 8000)
				{
					Main.chest[num31].Resize(newSize);
				}
				break;
			}
			case 156:
				if (Main.netMode == 2)
				{
					Point16 point = new Point16(reader.ReadInt16(), reader.ReadInt16());
					int itemType = reader.ReadInt16();
					if (TileEntity.TryGetAt<TELeashedEntityAnchorWithItem>(point.X, point.Y, out var result))
					{
						result.InsertItem(itemType);
					}
				}
				break;
			case 158:
				if (Main.netMode != 2)
				{
					byte b3 = reader.ReadByte();
					Main.player[b3].Spawn(PlayerSpawnContext.TeamSwap);
				}
				break;
			case 159:
				if (Main.netMode == 2)
				{
					int sectionX = reader.ReadUInt16();
					int sectionY = reader.ReadUInt16();
					NetMessage.SendSection(whoAmI, sectionX, sectionY);
				}
				break;
			case 160:
				if (Main.netMode != 2)
				{
					int num17 = reader.ReadInt16();
					Vector2 position = reader.ReadVector2();
					Main.item[num17].position = position;
				}
				break;
			case 161:
			{
				string text = reader.ReadString();
				Main.player[whoAmI].host = !string.IsNullOrWhiteSpace(Netplay.HostToken) && Netplay.HostToken == text;
				break;
			}
			default:
				if (Main.netMode == 2 && Netplay.Clients[whoAmI].State == 0)
				{
					NetMessage.BootPlayer(whoAmI, Lang.mp[2].ToNetworkText());
				}
				break;
			case 15:
			case 25:
			case 26:
			case 44:
			case 67:
			case 83:
			case 93:
				break;
			}
		}

		private static void ReadAccessoryVisibility(BinaryReader reader, bool[] hideVisibleAccessory)
		{
			ushort num = reader.ReadUInt16();
			for (int i = 0; i < hideVisibleAccessory.Length; i++)
			{
				hideVisibleAccessory[i] = (num & (1 << i)) != 0;
			}
		}

		private static void TrySendingItemArray(int plr, Item[] array, int slotStartIndex)
		{
			for (int i = 0; i < array.Length; i++)
			{
				NetMessage.TrySendData(5, -1, -1, null, plr, slotStartIndex + i);
			}
		}
	}
}
