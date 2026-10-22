using System;
using System.Collections.Generic;
using System.Linq;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon;
using Terraria.IO;
using Terraria.Localization;
using Terraria.Utilities;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Biomes
{
	public class DitherSnakePass : GenPass
	{
		public static readonly double[,] _bayerDither = new double[4, 4]
		{
			{ 0.0, 0.5, 0.125, 0.625 },
			{ 0.75, 0.25, 0.875, 0.375 },
			{ 0.1875, 0.6875, 0.0625, 0.5625 },
			{ 0.9375, 0.4375, 0.8125, 0.3125 }
		};

		public DitherSnakePass(string passName)
			: base(passName, 1.0)
		{
		}

		protected override void ApplyPass(GenerationProgress progress, GameConfiguration configuration)
		{
			progress.Message = Language.GetTextValue("WorldGeneration.DualDungeonsDitherSnake");
			GenVars.CurrentDungeon = 0;
			GenVars.CurrentDungeonGenVars.dungeonDitherSnake = CalculateDungeonDitherSnake(progress, 0.0, 0.02500000037252903);
			GenerateDungeonDitherSnake(progress, 0.02500000037252903, 0.5);
			GenVars.CurrentDungeon = 1;
			GenVars.CurrentDungeonGenVars.dungeonDitherSnake = CalculateDungeonDitherSnake(progress, 0.5, 0.5249999761581421);
			GenerateDungeonDitherSnake(progress, 0.5249999761581421, 1.0);
		}

		private void GenerateDungeonDitherSnake(GenerationProgress progress, double progressMin, double progressMax)
		{
			int num = 0;
			double num2 = GenVars.CurrentDungeonGenVars.dungeonDitherSnake.Count;
			foreach (DungeonControlLine item in GenVars.CurrentDungeonGenVars.dungeonDitherSnake)
			{
				progress.Set((double)num++ / num2, progressMin, progressMax);
				item.Paint(GenVars.CurrentDungeonGenVars.outerPotentialDungeonBounds.Hitbox);
			}
		}

		private DitherSnake CalculateDungeonDitherSnake(GenerationProgress progress, double progressMin, double progressMax)
		{
			DitherSnake ditherSnake = new DitherSnake();
			UnifiedRandom genRand = WorldGen.genRand;
			List<DungeonGenerationStyleData> dungeonGenerationStyles = GenVars.CurrentDungeonGenVars.dungeonGenerationStyles;
			DungeonBounds outerPotentialDungeonBounds = GenVars.CurrentDungeonGenVars.outerPotentialDungeonBounds;
			double num = (double)Main.maxTilesX / 4200.0;
			int num2 = (int)(20.0 * num);
			int count = dungeonGenerationStyles.Count;
			double num3 = 1.0 / (double)(num2 - 1);
			double num4 = 1.0 / (double)(count - 1);
			double num5 = (double)outerPotentialDungeonBounds.Height / (double)count;
			double num6 = 1.0 - DungeonControlLine.NormalizedDistanceSafeFromDither;
			double num7 = num5 / 2.0 * (1.0 + num6 / 2.0) - 1.0;
			double num8 = num7 - 0.1 * num * num5;
			double num9 = 0.05;
			double num10 = Utils.Remap(num, 1.0, 2.0, 1.0, 1.5);
			double num11 = num7 + num5 / 2.0 * num10;
			double num12 = num7;
			Vector2D vector2D = new Vector2D((double)outerPotentialDungeonBounds.X + num11, (double)outerPotentialDungeonBounds.Y + num12);
			Vector2D vector2D2 = new Vector2D((double)outerPotentialDungeonBounds.Width - num11 * 2.0, (double)outerPotentialDungeonBounds.Height - num12 * 2.0);
			double num13 = Math.Min(num7 - num8, vector2D2.X * num3 / 2.0);
			_ = dungeonGenerationStyles[0];
			double startRadius = num7;
			Vector2D start = Vector2D.Zero;
			int num14 = num2 * count;
			int num15 = num14 / dungeonGenerationStyles.Count;
			double num16 = num7;
			double num17 = num7;
			for (int i = 0; i < num14; i++)
			{
				progress.Set((double)i / (double)num14, progressMin, progressMax);
				int num18 = i % num2;
				int num19 = i / num2;
				double num20 = num3 * (double)num18;
				double y = num4 * (double)num19;
				int num21 = ((GenVars.CurrentDungeonGenVars.dungeonSide == DungeonSide.Left) ? 1 : (-1));
				if (num19 % 2 == 1)
				{
					num21 *= -1;
				}
				if (num21 < 0)
				{
					num20 = 1.0 - num20;
				}
				Vector2D vector2D3 = vector2D + vector2D2 * new Vector2D(num20, y);
				if (i == 0)
				{
					start = vector2D3;
					continue;
				}
				if (num19 == 0 && (vector2D3.X - (double)GenVars.CurrentDungeonGenVars.dungeonLocation) * (double)num21 < 0.0)
				{
					start = new Vector2D(GenVars.CurrentDungeonGenVars.dungeonLocation, vector2D3.Y);
					continue;
				}
				num16 = Utils.Lerp(Math.Max(num8, num16 - num13), Math.Min(num7, num16 + num13), genRand.NextDouble());
				num17 = Utils.Lerp(Math.Max(num8, num17 - num13), Math.Min(num7, num17 + num13), genRand.NextDouble());
				double num22 = (num16 + num17) / 2.0;
				vector2D3.Y += (num16 - num17) / 2.0;
				int num23 = i / num15;
				DungeonGenerationStyleData style = dungeonGenerationStyles[num23];
				DungeonControlLine line = new DungeonControlLine(start, vector2D3, startRadius, num22, num23, style);
				ditherSnake.Add(line);
				start = vector2D3;
				startRadius = num22;
				if (num18 == num2 - 1 && num19 != count - 1)
				{
					Vector2D vector2D4 = vector2D3;
					Vector2D value = vector2D3 + vector2D2 * new Vector2D(0.0, num4);
					Vector2D center = Vector2D.Lerp(vector2D4, value, 0.5);
					for (double num24 = num9; num24 < 0.5; num24 += num9)
					{
						vector2D3 = vector2D4.RotatedBy(Math.PI * 2.0 * num24 * (double)num21, center);
						vector2D3.X = Utils.Lerp(vector2D3.X, center.X, 1.0 - num10);
						line = new DungeonControlLine(start, vector2D3, startRadius, num22, num23, style)
						{
							CurveLine = true
						};
						ditherSnake.Add(line);
						start = vector2D3;
						startRadius = num22;
					}
					i++;
				}
			}
			ditherSnake.SetTangents();
			int num25 = ((GenVars.CurrentDungeonGenVars.dungeonSide == DungeonSide.Left) ? 1 : (-1));
			ditherSnake.First().StartTangent = Vector2D.UnitX * num25;
			ditherSnake.Last().EndTangent = Vector2D.UnitX * num25;
			ditherSnake.AdjustTangentsToPreventSelfIntersection();
			return ditherSnake;
		}
	}
}
