using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;

namespace Terraria.Graphics
{
	public class VertexStrip
	{
		public delegate Color StripColorFunction(float progressOnStrip);

		public delegate float StripHalfWidthFunction(float progressOnStrip);

		private struct CustomVertexInfo : IVertexType
		{
			public Vector2 Position;

			public Color Color;

			public Vector3 TexCoord;

			private static VertexDeclaration _vertexDeclaration = new VertexDeclaration(new VertexElement(0, VertexElementFormat.Vector2, VertexElementUsage.Position, 0), new VertexElement(8, VertexElementFormat.Color, VertexElementUsage.Color, 0), new VertexElement(12, VertexElementFormat.Vector3, VertexElementUsage.TextureCoordinate, 0));

			public VertexDeclaration VertexDeclaration => _vertexDeclaration;

			public CustomVertexInfo(Vector2 position, Color color, Vector3 texCoord)
			{
				Position = position;
				Color = color;
				TexCoord = texCoord;
			}
		}

		private CustomVertexInfo[] _vertices = new CustomVertexInfo[1];

		private int _vertexAmountCurrentlyMaintained;

		private short[] _indices = new short[1];

		private int _indicesAmountCurrentlyMaintained;

		private List<Vector2> _temporaryPositionsCache = new List<Vector2>();

		private List<float> _temporaryRotationsCache = new List<float>();

		public void Reset(int expectedVertexCount = 0)
		{
			_vertexAmountCurrentlyMaintained = 0;
			_indicesAmountCurrentlyMaintained = 0;
			if (_vertices.Length < expectedVertexCount)
			{
				Array.Resize(ref _vertices, expectedVertexCount);
			}
		}

		public void PrepareStrip(Vector2[] positions, float[] rotations, StripColorFunction colorFunction, StripHalfWidthFunction widthFunction, Vector2 offsetForAllPositions = default(Vector2), int? expectedVertexPairsAmount = null, bool includeBacksides = false)
		{
			int num = positions.Length;
			Reset(num * 2);
			int num2 = num;
			if (expectedVertexPairsAmount.HasValue)
			{
				num2 = expectedVertexPairsAmount.Value;
			}
			for (int i = 0; i < num && !(positions[i] == Vector2.Zero); i++)
			{
				Vector2 pos = positions[i] + offsetForAllPositions;
				float rot = MathHelper.WrapAngle(rotations[i]);
				float progressOnStrip = (float)i / (float)(num2 - 1);
				AddVertexPair(colorFunction, widthFunction, pos, rot, progressOnStrip);
			}
			PrepareIndices(includeBacksides);
		}

		public void PrepareStripWithProceduralPadding(Vector2[] positions, float[] rotations, StripColorFunction colorFunction, StripHalfWidthFunction widthFunction, Vector2 offsetForAllPositions = default(Vector2), bool includeBacksides = false, bool tryStoppingOddBug = true)
		{
			_temporaryPositionsCache.Clear();
			_temporaryRotationsCache.Clear();
			for (int i = 0; i < positions.Length && !(positions[i] == Vector2.Zero); i++)
			{
				Vector2 vector = positions[i];
				float num = MathHelper.WrapAngle(rotations[i]);
				_temporaryPositionsCache.Add(vector);
				_temporaryRotationsCache.Add(num);
				if (i + 1 >= positions.Length || !(positions[i + 1] != Vector2.Zero))
				{
					continue;
				}
				Vector2 vector2 = positions[i + 1];
				float num2 = MathHelper.WrapAngle(rotations[i + 1]);
				int num3 = (int)(Math.Abs(MathHelper.WrapAngle(num2 - num)) / ((float)Math.PI / 12f));
				if (num3 == 0)
				{
					continue;
				}
				float num4 = vector.Distance(vector2);
				Vector2 value = vector + num.ToRotationVector2() * num4;
				Vector2 value2 = vector2 + num2.ToRotationVector2() * (0f - num4);
				int num5 = num3 + 2;
				float num6 = 1f / (float)num5;
				Vector2 target = vector;
				for (float num7 = num6; num7 < 1f; num7 += num6)
				{
					Vector2 vector3 = Vector2.CatmullRom(value, vector, vector2, value2, num7);
					float num8 = MathHelper.WrapAngle(vector3.DirectionTo(target).ToRotation());
					if (float.IsNaN(num8))
					{
						num8 = _temporaryRotationsCache.Last();
					}
					_temporaryPositionsCache.Add(vector3);
					_temporaryRotationsCache.Add(num8);
					target = vector3;
				}
			}
			Reset(_temporaryPositionsCache.Count * 2);
			int count = _temporaryPositionsCache.Count;
			Vector2 zero = Vector2.Zero;
			for (int j = 0; j < count && (!tryStoppingOddBug || !(_temporaryPositionsCache[j] == zero)); j++)
			{
				Vector2 pos = _temporaryPositionsCache[j] + offsetForAllPositions;
				float rot = _temporaryRotationsCache[j];
				float progressOnStrip = (float)j / (float)(count - 1);
				AddVertexPair(colorFunction, widthFunction, pos, rot, progressOnStrip);
			}
			PrepareIndices(includeBacksides);
		}

		public void PrepareIndices(bool includeBacksides)
		{
			int num = _vertexAmountCurrentlyMaintained / 2 - 1;
			int num2 = 6 + includeBacksides.ToInt() * 6;
			int num3 = (_indicesAmountCurrentlyMaintained = num * num2);
			if (_indices.Length < num3)
			{
				Array.Resize(ref _indices, num3);
			}
			for (short num4 = 0; num4 < num; num4++)
			{
				short num5 = (short)(num4 * num2);
				int num6 = num4 * 2;
				_indices[num5] = (short)num6;
				_indices[num5 + 1] = (short)(num6 + 1);
				_indices[num5 + 2] = (short)(num6 + 2);
				_indices[num5 + 3] = (short)(num6 + 2);
				_indices[num5 + 4] = (short)(num6 + 1);
				_indices[num5 + 5] = (short)(num6 + 3);
				if (includeBacksides)
				{
					_indices[num5 + 6] = (short)(num6 + 2);
					_indices[num5 + 7] = (short)(num6 + 1);
					_indices[num5 + 8] = (short)num6;
					_indices[num5 + 9] = (short)(num6 + 2);
					_indices[num5 + 10] = (short)(num6 + 3);
					_indices[num5 + 11] = (short)(num6 + 1);
				}
			}
		}

		public void AddVertexPair(StripColorFunction colorFunction, StripHalfWidthFunction widthFunction, Vector2 pos, float rot, float progressOnStrip)
		{
			Color vertexColor = colorFunction(progressOnStrip);
			float num = widthFunction(progressOnStrip);
			Vector2 vector = MathHelper.WrapAngle(rot - (float)Math.PI / 2f).ToRotationVector2() * num;
			AddVertexPair(pos + vector, pos - vector, progressOnStrip, vertexColor);
		}

		public void AddVertexPair(Vector2 a, Vector2 b, Vector3 uvA, Vector3 uvB, Color vertexColor)
		{
			while (_vertexAmountCurrentlyMaintained + 1 >= _vertices.Length)
			{
				Array.Resize(ref _vertices, _vertices.Length * 2);
			}
			Vector2.Distance(a, b);
			_vertices[_vertexAmountCurrentlyMaintained].Position = a;
			_vertices[_vertexAmountCurrentlyMaintained + 1].Position = b;
			_vertices[_vertexAmountCurrentlyMaintained].TexCoord = uvA;
			_vertices[_vertexAmountCurrentlyMaintained + 1].TexCoord = uvB;
			_vertices[_vertexAmountCurrentlyMaintained].Color = vertexColor;
			_vertices[_vertexAmountCurrentlyMaintained + 1].Color = vertexColor;
			_vertexAmountCurrentlyMaintained += 2;
		}

		public void AddVertexPair(Vector2 a, Vector2 b, float uv_x, Color vertexColor)
		{
			while (_vertexAmountCurrentlyMaintained + 1 >= _vertices.Length)
			{
				Array.Resize(ref _vertices, _vertices.Length * 2);
			}
			float num = Vector2.Distance(a, b);
			_vertices[_vertexAmountCurrentlyMaintained].Position = a;
			_vertices[_vertexAmountCurrentlyMaintained + 1].Position = b;
			_vertices[_vertexAmountCurrentlyMaintained].TexCoord = new Vector3(uv_x, num, num);
			_vertices[_vertexAmountCurrentlyMaintained + 1].TexCoord = new Vector3(uv_x, 0f, num);
			_vertices[_vertexAmountCurrentlyMaintained].Color = vertexColor;
			_vertices[_vertexAmountCurrentlyMaintained + 1].Color = vertexColor;
			_vertexAmountCurrentlyMaintained += 2;
		}

		public void AddVertexPair(Vector2 v1, Vector2 v2, float uv_x, Color color1, Color color2)
		{
			while (_vertexAmountCurrentlyMaintained + 1 >= _vertices.Length)
			{
				Array.Resize(ref _vertices, _vertices.Length * 2);
			}
			float num = Vector2.Distance(v1, v2);
			_vertices[_vertexAmountCurrentlyMaintained++] = new CustomVertexInfo(v1, color1, new Vector3(uv_x, num, num));
			_vertices[_vertexAmountCurrentlyMaintained++] = new CustomVertexInfo(v2, color2, new Vector3(uv_x, 0f, num));
		}

		public void DrawTrail()
		{
			if (_vertexAmountCurrentlyMaintained >= 3)
			{
				GraphicsDevice graphicsDevice = Main.instance.GraphicsDevice;
				VertexBufferBinding[] vertexBuffers = graphicsDevice.GetVertexBuffers();
				IndexBuffer indices = graphicsDevice.Indices;
				graphicsDevice.DrawUserIndexedPrimitives(PrimitiveType.TriangleList, _vertices, 0, _vertexAmountCurrentlyMaintained, _indices, 0, _indicesAmountCurrentlyMaintained / 3);
				graphicsDevice.SetVertexBuffers(vertexBuffers);
				graphicsDevice.Indices = indices;
			}
		}
	}
}
