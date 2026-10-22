using System.Collections.Generic;
using System.IO;
using Microsoft.Xna.Framework;
using ReLogic.Content;
using ReLogic.Threading;
using Terraria.Audio;
using Terraria.DataStructures;
using Terraria.GameContent;
using Terraria.GameInput;
using Terraria.Graphics.Renderers;
using Terraria.Graphics.Shaders;
using Terraria.IO;
using Terraria.Localization;
using Terraria.Testing.Cloning;
using Terraria.UI;

namespace Terraria.Testing
{
	public class StateSnapshot
	{
		private sealed class WorldHeaderComponent : StateSnapshotDefinition.Component
		{
			public WorldHeaderComponent()
				: base("WorldHeaderComponent")
			{
			}

			public override object Backup(DeepCloneContext ctx)
			{
				WorldFile.SetTempToOngoing();
				MemoryStream memoryStream = new MemoryStream();
				WorldFile.SaveWorldFlags(new BinaryWriter(memoryStream));
				return memoryStream;
			}

			public override void Restore(object clone, DeepCloneContext ctx)
			{
				MemoryStream obj = (MemoryStream)clone;
				obj.Position = 0L;
				WorldFile.LoadWorldFlags(new BinaryReader(obj), 326);
				WorldFile.SetOngoingToTemps();
			}
		}

		private sealed class TilesComponent : StateSnapshotDefinition.Component
		{
			private struct TileData
			{
				private long a;

				private long b;
			}

			public TilesComponent()
				: base("Main.tile")
			{
			}

			public override object Backup(DeepCloneContext ctx)
			{
				TileData[] data = new TileData[Main.maxTilesX * Main.maxTilesY];
				FastParallel.For(0, Main.maxTilesX, delegate(int x0, int x1, object _)
				{
					BackupRange(x0, x1, data);
				});
				return data;
			}

			private unsafe static void BackupRange(int x0, int x1, TileData[] data)
			{
				int maxTilesY = Main.maxTilesY;
				Tile[,] tile = Main.tile;
				fixed (TileData* ptr = &data[x0 * maxTilesY])
				{
					TileData* ptr2 = ptr;
					for (int i = x0; i < x1; i++)
					{
						for (int j = 0; j < maxTilesY; j++)
						{
							fixed (ushort* type = &tile[i, j].type)
							{
								*(ptr2++) = *(TileData*)type;
							}
						}
					}
				}
			}

			public override void Restore(object clone, DeepCloneContext ctx)
			{
				TileData[] data = (TileData[])clone;
				FastParallel.For(0, Main.maxTilesX, delegate(int x0, int x1, object _)
				{
					RestoreRange(x0, x1, data);
				});
			}

			private unsafe static void RestoreRange(int x0, int x1, TileData[] data)
			{
				int maxTilesY = Main.maxTilesY;
				Tile[,] tile = Main.tile;
				fixed (TileData* ptr = &data[x0 * maxTilesY])
				{
					TileData* ptr2 = ptr;
					for (int i = x0; i < x1; i++)
					{
						for (int j = 0; j < maxTilesY; j++)
						{
							fixed (ushort* type = &tile[i, j].type)
							{
								*(TileData*)type = *(ptr2++);
							}
						}
					}
				}
			}
		}

		public static readonly StateSnapshotDefinition Gameplay;

		public static readonly StateSnapshotDefinition Input;

		private readonly StateSnapshotDefinition _definition;

		private readonly object[] _clones;

		static StateSnapshot()
		{
			Gameplay = new StateSnapshotDefinition();
			Input = new StateSnapshotDefinition();
			DeepCloning.AddImmutableType<LocalizedText>();
			DeepCloning.AddImmutableType<NetworkText>();
			DeepCloning.AddImmutableType<ItemTooltip>();
			DeepCloning.AddImmutableType<LegacySoundStyle>();
			DeepCloning.AddImmutableType<ShaderData>();
			DeepCloning.AddImmutableType(typeof(Asset<>));
			BackupRestoreHelper<Player>.Register(new ActiveCloneHelper<Player>((Player p) => p.active, delegate(Player p)
			{
				p.active = false;
			}));
			BackupRestoreHelper<NPC>.Register(new ActiveCloneHelper<NPC>((NPC p) => p.active, delegate(NPC p)
			{
				p.active = false;
			}));
			BackupRestoreHelper<Projectile>.Register(new ActiveCloneHelper<Projectile>((Projectile p) => p.active, delegate(Projectile p)
			{
				p.active = false;
			}));
			BackupRestoreHelper<WorldItem>.Register(new ActiveCloneHelper<WorldItem>((WorldItem p) => !p.IsAir, delegate(WorldItem p)
			{
				p.TurnToAir();
			}));
			BackupRestoreHelper<Dust>.Register(new ActiveCloneHelper<Dust>((Dust p) => p.active, delegate(Dust p)
			{
				p.active = false;
			}));
			BackupRestoreHelper<Gore>.Register(new ActiveCloneHelper<Gore>((Gore p) => p.active, delegate(Gore p)
			{
				p.active = false;
			}));
			BackupRestoreHelper<CombatText>.Register(new ActiveCloneHelper<CombatText>((CombatText p) => p.active, delegate(CombatText p)
			{
				p.active = false;
			}));
			BackupRestoreHelper<PopupText>.Register(new ActiveCloneHelper<PopupText>((PopupText p) => p.active, delegate(PopupText p)
			{
				p.active = false;
			}));
			BackupRestoreHelper<Chest>.Register(new ChestCloneHelper());
			AddGameplayComponents();
			AddInputComponents();
		}

		private static void AddGameplayComponents()
		{
			Gameplay.Add(new WorldHeaderComponent());
			Gameplay.Add(new TilesComponent());
			Gameplay.AddRef("Main.sectionManager", () => Main.sectionManager, delegate(WorldSections v)
			{
				Main.sectionManager = v;
			});
			Liquid.AddGameplaySnapshotComponents();
			Gameplay.AddCollection("Main.player", Main.player);
			Gameplay.AddCollection("Main.npc", Main.npc);
			Gameplay.AddCollection("Main.projectile", Main.projectile);
			Gameplay.AddCollection("Main.item", Main.item);
			Gameplay.AddCollection("Main.dust", Main.dust);
			Gameplay.AddCollection("Main.gore", Main.gore);
			Gameplay.AddCollection("Main.combatText", Main.combatText);
			Gameplay.AddCollection("PopupText.popupText", PopupText.popupText);
			Gameplay.AddCollection("Main.chest", Main.chest);
			Gameplay.AddRef("Main.MouseItem", () => Main.mouseItem, delegate(Item v)
			{
				Main.mouseItem = v;
			});
			Projectile.AddGameplaySnapshotComponents();
			Main.AddGameplaySnapshotComponents();
			Chest.AddGameplaySnapshotComponents();
			ActiveSections.AddGameplaySnapshotComponents();
			Gameplay.AddRef("NPC.spawnSlotProtected", () => NPC.spawnSlotProtected, delegate(int[] v)
			{
				NPC.spawnSlotProtected = v;
			});
			Gameplay.AddRef("NPC.lazyNPCOwnedProjectileSearchArray", () => NPC.lazyNPCOwnedProjectileSearchArray, delegate(int[] v)
			{
				NPC.lazyNPCOwnedProjectileSearchArray = v;
			});
			Gameplay.AddRef("NPC.ShimmeredTownNPCs", () => NPC.ShimmeredTownNPCs, delegate(bool[] v)
			{
				NPC.ShimmeredTownNPCs = v;
			});
			Gameplay.AddRef("NPC.npcsFoundForCheckActive", () => NPC.npcsFoundForCheckActive, delegate(bool[] v)
			{
				NPC.npcsFoundForCheckActive = v;
			});
			Gameplay.AddRef("TileEntity.ByID", () => TileEntity.ByID, delegate(Dictionary<int, TileEntity> v)
			{
				TileEntity.ByID = v;
			});
			Gameplay.AddRef("TileEntity.ByPosition", () => TileEntity.ByPosition, delegate(Dictionary<Point16, TileEntity> v)
			{
				TileEntity.ByPosition = v;
			});
			Gameplay.AddRef("TileEntity.UpdateEntities", () => TileEntity.UpdateEntities, delegate(List<TileEntity> v)
			{
				TileEntity.UpdateEntities = v;
			});
			Gameplay.AddVal("TileEntity.TileEntitiesNextID", () => TileEntity.TileEntitiesNextID, delegate(int v)
			{
				TileEntity.TileEntitiesNextID = v;
			});
			LeashedEntity.AddGameplaySnapshotComponents();
			Gameplay.AddRef("NPC.RevengeManager", () => NPC.RevengeManager, delegate(CoinLossRevengeSystem v)
			{
				NPC.RevengeManager = v;
			});
			Gameplay.AddVal("NPC.taxCollector", () => NPC.taxCollector, delegate(bool v)
			{
				NPC.taxCollector = v;
			});
			Gameplay.AddVal("NPC.offSetDelayTime", () => NPC.offSetDelayTime, delegate(int v)
			{
				NPC.offSetDelayTime = v;
			});
			Gameplay.AddVal("NPC.empressRageMode", () => NPC.empressRageMode, delegate(bool v)
			{
				NPC.empressRageMode = v;
			});
			Gameplay.AddVal("NPC.brainOfGravity", () => NPC.brainOfGravity, delegate(int v)
			{
				NPC.brainOfGravity = v;
			});
			Gameplay.AddVal("NPC.golemBoss", () => NPC.golemBoss, delegate(int v)
			{
				NPC.golemBoss = v;
			});
			Gameplay.AddVal("NPC.plantBoss", () => NPC.plantBoss, delegate(int v)
			{
				NPC.plantBoss = v;
			});
			Gameplay.AddVal("NPC.crimsonBoss", () => NPC.crimsonBoss, delegate(int v)
			{
				NPC.crimsonBoss = v;
			});
			Gameplay.AddVal("NPC.deerclopsBoss", () => NPC.deerclopsBoss, delegate(int v)
			{
				NPC.deerclopsBoss = v;
			});
			Gameplay.AddVal("NPC.freeCake", () => NPC.freeCake, delegate(bool v)
			{
				NPC.freeCake = v;
			});
			Gameplay.AddVal("NPC.mechQueen", () => NPC.mechQueen, delegate(int v)
			{
				NPC.mechQueen = v;
			});
			Gameplay.AddVal("NPC.MoonLordCountdown", () => NPC.MoonLordCountdown, delegate(int v)
			{
				NPC.MoonLordCountdown = v;
			});
			Gameplay.AddVal("NPC.waveKills", () => NPC.waveKills, delegate(float v)
			{
				NPC.waveKills = v;
			});
			Gameplay.AddVal("NPC.waveNumber", () => NPC.waveNumber, delegate(int v)
			{
				NPC.waveNumber = v;
			});
			Gameplay.AddVal("NPC.totalInvasionPoints", () => NPC.totalInvasionPoints, delegate(float v)
			{
				NPC.totalInvasionPoints = v;
			});
			Gameplay.AddVal("NPC.fireFlyFriendly", () => NPC.fireFlyFriendly, delegate(int v)
			{
				NPC.fireFlyFriendly = v;
			});
			Gameplay.AddVal("NPC.fireFlyChance", () => NPC.fireFlyChance, delegate(int v)
			{
				NPC.fireFlyChance = v;
			});
			Gameplay.AddVal("NPC.fireFlyMultiple", () => NPC.fireFlyMultiple, delegate(int v)
			{
				NPC.fireFlyMultiple = v;
			});
			Gameplay.AddVal("NPC.butterflyChance", () => NPC.butterflyChance, delegate(int v)
			{
				NPC.butterflyChance = v;
			});
			Gameplay.AddVal("NPC.stinkBugChance", () => NPC.stinkBugChance, delegate(int v)
			{
				NPC.stinkBugChance = v;
			});
			Gameplay.AddVal("CurrentFrameFlags.ActivePlayersCount", () => Main.CurrentFrameFlags.ActivePlayersCount, delegate(int v)
			{
				Main.CurrentFrameFlags.ActivePlayersCount = v;
			});
			Gameplay.AddVal("CurrentFrameFlags.SleepingPlayersCount", () => Main.CurrentFrameFlags.SleepingPlayersCount, delegate(int v)
			{
				Main.CurrentFrameFlags.SleepingPlayersCount = v;
			});
			Gameplay.AddVal("CurrentFrameFlags.AnyActiveBossNPC", () => Main.CurrentFrameFlags.AnyActiveBossNPC, delegate(bool v)
			{
				Main.CurrentFrameFlags.AnyActiveBossNPC = v;
			});
			Gameplay.AddVal("CurrentFrameFlags.HadAnActiveInteractableProjectile", () => Main.CurrentFrameFlags.HadAnActiveInteractableProjectile, delegate(bool v)
			{
				Main.CurrentFrameFlags.HadAnActiveInteractableProjectile = v;
			});
			Gameplay.AddVal("WorldGen.meteorShowerCount", () => WorldGen.meteorShowerCount, delegate(int v)
			{
				WorldGen.meteorShowerCount = v;
			});
			Gameplay.AddVal("WorldGen.spawnEye", () => WorldGen.spawnEye, delegate(bool v)
			{
				WorldGen.spawnEye = v;
			});
			Gameplay.AddVal("WorldGen.spawnHardBoss", () => WorldGen.spawnHardBoss, delegate(int v)
			{
				WorldGen.spawnHardBoss = v;
			});
			Gameplay.AddVal("WorldGen.spawnMeteor", () => WorldGen.spawnMeteor, delegate(bool v)
			{
				WorldGen.spawnMeteor = v;
			});
			Gameplay.AddVal("WorldGen.npcSpawnDelay", () => WorldGen.npcSpawnDelay, delegate(int v)
			{
				WorldGen.npcSpawnDelay = v;
			});
			Gameplay.AddVal("WorldGen.prioritizedTownNPCType", () => WorldGen.prioritizedTownNPCType, delegate(int v)
			{
				WorldGen.prioritizedTownNPCType = v;
			});
			Gameplay.AddVal("WorldGen.homelessSpawnTimeout", () => WorldGen.homelessSpawnTimeout, delegate(int v)
			{
				WorldGen.homelessSpawnTimeout = v;
			});
			Gameplay.AddVal("WorldGen.tEvil", () => WorldGen.tEvil, delegate(byte v)
			{
				WorldGen.tEvil = v;
			});
			Gameplay.AddVal("WorldGen.tBlood", () => WorldGen.tBlood, delegate(byte v)
			{
				WorldGen.tBlood = v;
			});
			Gameplay.AddVal("WorldGen.tGood", () => WorldGen.tGood, delegate(byte v)
			{
				WorldGen.tGood = v;
			});
			Gameplay.AddRef("PressurePlateHelper.PressurePlatesPressed", () => PressurePlateHelper.PressurePlatesPressed, delegate(Dictionary<Point, bool[]> v)
			{
				PressurePlateHelper.PressurePlatesPressed = v;
			});
			NPCDamageTracker.AddGameplaySnapshotComponents();
			Gameplay.AddRef("Main.ParticleSystem_World_OverPlayers", () => Main.ParticleSystem_World_OverPlayers, delegate(ParticleRenderer v)
			{
				Main.ParticleSystem_World_OverPlayers = v;
			});
			Gameplay.AddRef("Main.ParticleSystem_World_BehindPlayers", () => Main.ParticleSystem_World_BehindPlayers, delegate(ParticleRenderer v)
			{
				Main.ParticleSystem_World_BehindPlayers = v;
			});
			Lighting.AddGameplaySnapshotComponents();
		}

		private static void AddInputComponents()
		{
			Input.AddVal("Main.mouseX", () => Main.mouseX, delegate(int v)
			{
				Main.mouseX = v;
			});
			Input.AddVal("Main.mouseY", () => Main.mouseY, delegate(int v)
			{
				Main.mouseY = v;
			});
			Input.AddVal("Main.lastMouseX", () => Main.lastMouseX, delegate(int v)
			{
				Main.lastMouseX = v;
			});
			Input.AddVal("Main.lastMouseY", () => Main.lastMouseY, delegate(int v)
			{
				Main.lastMouseY = v;
			});
			Input.AddVal("Main.mouseLeft", () => Main.mouseLeft, delegate(bool v)
			{
				Main.mouseLeft = v;
			});
			Input.AddVal("Main.mouseRight", () => Main.mouseRight, delegate(bool v)
			{
				Main.mouseRight = v;
			});
			Input.AddVal("Main.screenPosition", () => Main.screenPosition, delegate(Vector2 v)
			{
				Main.screenPosition = v;
			});
			Input.AddVal("Main.screenWidth", () => Main.screenWidth, delegate(int v)
			{
				Main.screenWidth = v;
			});
			Input.AddVal("Main.screenHeight", () => Main.screenHeight, delegate(int v)
			{
				Main.screenHeight = v;
			});
			Input.AddVal("Main.SmartCursorWanted_Mouse", () => Main.SmartCursorWanted_Mouse, delegate(bool v)
			{
				Main.SmartCursorWanted_Mouse = v;
			});
			Input.AddVal("Main.SmartCursorWanted_GamePad", () => Main.SmartCursorWanted_GamePad, delegate(bool v)
			{
				Main.SmartCursorWanted_GamePad = v;
			});
			Input.AddVal("Main.HoveringOverAnNPC", () => Main.HoveringOverAnNPC, delegate(bool v)
			{
				Main.HoveringOverAnNPC = v;
			});
			Input.AddVal("Main.drawingPlayerChat", () => Main.drawingPlayerChat, delegate(bool v)
			{
				Main.drawingPlayerChat = v;
			});
			Input.AddVal("Main.editSign", () => Main.editSign, delegate(bool v)
			{
				Main.editSign = v;
			});
			Input.AddVal("Main.editChest", () => Main.editChest, delegate(bool v)
			{
				Main.editChest = v;
			});
			Input.AddVal("Main.blockInput", () => Main.blockInput, delegate(bool v)
			{
				Main.blockInput = v;
			});
			Input.AddVal("FocusHelper.IsSelectedApplication", () => FocusHelper.IsSelectedApplication, delegate(bool v)
			{
				FocusHelper.IsSelectedApplication = v;
			});
			Input.AddVal("Main.LocalPlayer.delayUseItem", () => Main.LocalPlayer.delayUseItem, delegate(bool v)
			{
				Main.LocalPlayer.delayUseItem = v;
			});
			PlayerInput.AddInputSnapshotComponents();
			LockOnHelper.AddInputSnapshotComponents();
		}

		public StateSnapshot(StateSnapshotDefinition definition, object[] clones)
		{
			_definition = definition;
			_clones = clones;
		}

		public void Restore()
		{
			_definition.Restore(_clones);
		}
	}
}
