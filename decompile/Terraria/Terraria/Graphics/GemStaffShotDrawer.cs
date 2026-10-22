using Microsoft.Xna.Framework;
using Terraria.Graphics.Shaders;

namespace Terraria.Graphics
{
	public struct GemStaffShotDrawer
	{
		private static VertexStrip _vertexStrip = new VertexStrip();

		private Color _trailColor1;

		private Color _trailColor2;

		private Color _trailColor3;

		public void Draw(Projectile proj, string gameShaderName, Color trailColor1, Color trailColor2, Color trailColor3, float intensity = 2f)
		{
			MiscShaderData miscShaderData = GameShaders.Misc[gameShaderName];
			_trailColor1 = trailColor1;
			_trailColor2 = trailColor2;
			_trailColor3 = trailColor3;
			miscShaderData.UseSaturation(-2.8f);
			miscShaderData.UseOpacity(intensity);
			miscShaderData.Apply();
			_vertexStrip.PrepareStripWithProceduralPadding(proj.oldPos, proj.oldRot, StripColors, StripWidth, -Main.screenPosition + proj.Size / 2f);
			_vertexStrip.DrawTrail();
			Main.pixelShader.CurrentTechnique.Passes[0].Apply();
		}

		private Color StripColors(float progressOnStrip)
		{
			Color value = Color.Lerp(_trailColor1, _trailColor2, progressOnStrip);
			return Color.Lerp(_trailColor3, value, Utils.GetLerpValue(-0.2f, 0.5f, progressOnStrip, clamped: true)) * (1f - Utils.GetLerpValue(0f, 0.98f, progressOnStrip));
		}

		private float StripWidth(float progressOnStrip)
		{
			return 16f;
		}
	}
}
