"""Spec of the sky / background converter: the drawing methods (terraria-app, `main_backgrounds_gen.rs`) and the
simulation methods (terraria-sim, `sky_gen.rs`).   python cs_bg.py cs_bg_draw.py > ...rs   (MODE=draw|sim in the environment)"""
import os
import sqlite3
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cs_bg import CsError, conv, is_num, promote, snake, lname, RUST_T  # noqa: E402
import cs_bg_common as common  # noqa: E402

ROOT = r'C:\Users\user\Documents\_Projects_\VAEL Workspace\VAEL - TERRARIA'
MAIN_CS = ROOT + r'\decompile\Terraria\Terraria\Main.cs'
WORLDGEN_CS = ROOT + r'\decompile\Terraria\Terraria\WorldGen.cs'
MODE = os.environ.get('MODE', 'draw')
db = sqlite3.connect(ROOT + r'\db\terraria.sqlite')


def lines_of(type_name, name):
    r = db.execute("SELECT m.start_line, m.end_line FROM port_methods m WHERE m.type_name=? AND m.name=? ORDER BY m.start_line", (type_name, name)).fetchall()
    if len(r) != 1:
        raise CsError('%s.%s: %d matches' % (type_name, name, len(r)))
    return r[0]


G = 'ctx.g' if MODE == 'draw' else 'g'

# ---------------------------------------------------------------------------------------------------------------------
# functions to convert: (C# name, class, mode, params, ret)
# ---------------------------------------------------------------------------------------------------------------------
F = lambda n, cls='Main', params=(), ret='void', vis='pub(crate)': dict(cs=n, cls=cls, params=list(params), ret=ret, vis=vis)
P = lambda n, t, ref=False: (n, t, ref)
BG4 = [P('backgroundTopMagicNumber', 'float'), P('bgGlobalScaleMultiplier', 'float'), P('pushBGTopHack', 'int'), P('bgTexIndexes', 'int[]')]

DRAW_FUNCS = [
    F('DrawSurfaceBG_DrawChangeOverlay', params=[P('backgroundAreaId', 'int')]),
    F('DrawSurfaceBG_GetLayerYOffset', params=[P('bgTextureIndex', 'int')], ret='int'),
    F('DrawSurfaceBG_DrawBackMountainsLayer', params=[P('bgTextureIndex', 'int')]),
    F('SetBackgroundOffsets', params=[P('backgroundID', 'int'), P('backgroundTopMagicNumber', 'float'), P('pushBGTopHack', 'int')]),
    F('GetBackgroundRect', params=[P('backgroundTextureIndex', 'int')], ret='Rectangle?'),
    F('GetBackgroundCounter', ret='uint'),
    F('DrawSurfaceBG_BackMountainsStep1', params=[P('backgroundTopMagicNumber', 'float'), P('bgGlobalScaleMultiplier', 'float'), P('pushBGTopHack', 'int')]),
    F('DrawSurfaceBG_BackMountainsStep2', params=[P('pushBGTopHack', 'int')]),
    F('DrawSurfaceBG_Mushroom', params=BG4),
    F('DrawSurfaceBG_Crimson', params=BG4),
    F('DrawSurfaceBG_Snow', params=BG4),
    F('DrawSurfaceBG_Hallow', params=BG4),
    F('DrawSurfaceBG_GoodEvilDesert', params=BG4[:3] + [P('layerIndex', 'int')]),
    F('DrawSurfaceBG_Jungle', params=BG4),
    F('DrawSurfaceBG_Desert', params=BG4),
    F('DrawSurfaceBG_Corrupt', params=BG4),
    F('DrawSurfaceBG_Forest', params=BG4),
    F('DrawBackgroundBlackFill'),
    F('DrawSurfaceBG', params=[], ret='void'),
    F('DrawBG', params=[]),
    F('DrawUnderworldBackground', params=[P('zoomForPushUp', 'float')]),
    F('DrawUnderworldBackgroundLayer', params=[P('screenOffset', 'Vector2'), P('pushUp', 'float'), P('layerTextureIndex', 'int')]),
    F('DrawBackground'),
    F('DrawBackground_DrawMagmaLayer', params=[P('magmaLayer', 'double'), P('q1', 'int'), P('q2', 'int'), P('drawOffset', 'Vector2', True), P('backgroundColor', 'Vector3', True)]),
    F('GetRockTransitionPoint', ret='int'),
    F('DrawBackground_DrawRockLayer', params=[P('localShimmerAlpha', 'float'), P('magmaLayer', 'double'), P('q1', 'int'), P('q2', 'int'), P('drawOffset', 'Vector2', True), P('backgroundColor', 'Vector3', True)]),
    F('DrawBackground_DrawMagmaTransition', params=[P('drawOffset', 'Vector2', True), P('magmaTransition', 'bool'), P('backgroundColor', 'Vector3', True), P('backgroundWidth', 'int', True), P('diff', 'int')]),
    F('DrawBackground_DrawUnderworldBlackBox', params=[P('magmaLayer', 'double'), P('drawOffset', 'Vector2')]),
    F('DrawBackground_DirtBackground', params=[P('localShimmerAlpha', 'float'), P('q1', 'int'), P('q2', 'int'), P('drawOffset', 'Vector2', True), P('backgroundColor', 'Vector3', True)]),
    F('DrawBackground_SurfaceTransitionBackground', params=[P('localShimmerAlpha', 'float'), P('drawOffset', 'Vector2', True), P('backgroundColor', 'Vector3', True)]),
    F('DrawBackground_UpdateBackgroundStyles'),
    F('ComputeScAdj', params=[P('screenY', 'float'), P('screenHeight', 'float')], ret='float'),
    F('DrawBG_HandleBackgroundTransition', params=[P('newBackgroundStyle', 'int')]),
]
SIM_FUNCS = [
    dict(cs='SetBackColor', cls='Main', params=[P('info', 'InfoToSetBackColor'), P('sunColor', 'Color', True), P('moonColor', 'Color', True)], ret='void', vis='pub'),
    dict(cs='GetPreferredBGStyleForPlayer', cls='Main', params=[], ret='int', vis='pub'),
    dict(cs='DrawBackground_PickUndergroundBackgroundStyle', cls='Main', params=[P('magmaLayer', 'double')], ret='int', vis='pub'),
    dict(cs='setBG', cls='WorldGen', params=[P('bg', 'int'), P('style', 'int')], vis='pub'),
    dict(cs='SetForestBGSet', cls='WorldGen', noctx=True, params=[P('style', 'int'), P('mountainSet', 'int[]', True), P('treeSet', 'int[]', True)], vis='pub(crate)'),
    dict(cs='GetCurrentBackgroundOfStyle', cls='WorldGen', params=[P('backgroundStyle', 'int')], ret='int', vis='pub'),
    dict(cs='UpdateAtmosphereTransparencyToSkyColor', cls='Main', params=[P('y', 'float')], vis='pub'),
]
FILTER = os.environ.get('FILTER')
FUNCS = [f for f in (DRAW_FUNCS if MODE == 'draw' else SIM_FUNCS) if not FILTER or f['cls'] == FILTER]

OWN = {}  # C# name -> (rust name, [(type, byref)], ret, takes_ctx)
for f in DRAW_FUNCS + SIM_FUNCS:
    OWN[f['cs']] = (snake(f['cs']), [(p[1], p[2]) for p in f['params']], f.get('ret', 'void'), not f.get('noctx'))
# hand-written in main_backgrounds.rs / main_time.rs: name -> (rust path, param types, ret)
HAND = {
    'LoadBackground': ('load_background', [('int', False)], 'void'),
    'UpdateOceanWaterLineForAmbience': ('update_ocean_water_line_for_ambience', [], 'void'),
    'WallLightAt': ('wall_light_at', [('int', False), ('int', False)], 'bool'),
    'ShouldDrawBackgroundTileAt': ('should_draw_background_tile_at', [('int', False), ('int', False)], 'bool'),
    'DrawSurfaceBG_GetFogPower': ('draw_surface_bg_get_fog_power', [], 'float'),
    'ShouldDrawSurfaceBackground': ('should_draw_surface_background', [], 'bool'),
    'UpdateBGVisibility_BackLayer': ('update_bg_visibility_back_layer', [('null', False), ('null', False)], 'void'),
    'UpdateBGVisibility_FrontLayer': ('update_bg_visibility_front_layer', [('null', False), ('null', False)], 'void'),
    'DrawClouds_Distant': ('draw_clouds_distant', [('bool', False), ('float', False), ('int', False), ('float', False)], 'void'),
    'DrawClouds_Closer': ('draw_clouds_closer', [('float', False)], 'void'),
    'DrawClouds_Closest': ('draw_clouds_closest', [('float', False)], 'void'),
    'HorizonDrawSurfaceLayer': ('horizon_draw_surface_layer', [('int', False)], 'void'),
}
if MODE == 'sim':
    HAND = {}


def own_call(c, name, args):
    """Call of a method that is converted (OWN) or written by hand (HAND): the context argument goes first."""
    if name in OWN:
        rname, ptypes, ret, takes_ctx = OWN[name]
        cls = next(f['cls'] for f in DRAW_FUNCS + SIM_FUNCS if f['cs'] == name)
        in_sim = name in {f['cs'] for f in SIM_FUNCS}
        if MODE == 'draw' and in_sim:
            rname = ('terraria_sim::world_gen_bg::' if cls == 'WorldGen' else 'terraria_sim::sky_gen::') + rname
            ctxarg = 'ctx.g'
        elif MODE == 'sim' and in_sim:
            rname = ('crate::world_gen_bg::' if cls == 'WorldGen' else 'crate::sky_gen::') + rname
            ctxarg = 'g'
        else:
            ctxarg = 'ctx'
        if not takes_ctx:
            ctxarg = None
    else:
        rname, ptypes, ret = HAND[name]
        ptypes = [(t, False) for t, _ in ptypes]
        ctxarg = 'ctx'
    out = []
    for (code, t, kw), (pt, byref) in zip(args, ptypes):
        if pt == 'null':
            out.append('None')
        elif byref and not pt.endswith('[]'):
            if kw != 'ref':
                raise CsError('%s expects a ref argument' % name)
            out.append(code[2:-1] if code.startswith('(*') and code.endswith(')') else '&mut ' + code)
        elif pt.endswith('[]') and byref:
            out.append('&mut ' + code)
        elif pt.endswith('[]'):
            out.append('&%s.clone()' % code)
        elif is_num(pt):
            out.append(conv(code, t, pt))
        else:
            out.append(code)
    return ('%s(%s)' % (rname, ', '.join(([ctxarg] if ctxarg else []) + out)), ret)


def draw_call(c, sb, args):
    types = [a[1] for a in args]
    codes = [a[0] for a in args]
    for i, t in enumerate(types):
        if t == 'xfloat':  # a float temporary passed to a method: rounded to 32 bits
            codes[i], types[i] = conv(codes[i], t, 'float'), 'float'
    n = len(args)
    pre = ['let __tex = (%s).clone();' % codes[0]]
    vals = []
    for i in range(1, n):
        pre.append('let __a%d = %s;' % (i, codes[i]))
    if n == 3 and types[1] == 'Rectangle':
        call = 'ctx.sprite_batch.draw_rect(__t, __a1, None, __a2, 0.0_f32, Vector2::ZERO, SpriteEffects::NONE)'
    elif n == 3 and types[1] == 'Vector2':
        call = 'ctx.sprite_batch.draw_simple(__t, __a1, __a2)'
    elif n == 4 and types[1] == 'Vector2':
        src = '__a2' if types[2] == 'Rectangle?' else 'Some(__a2)'
        call = 'ctx.sprite_batch.draw(__t, __a1, %s, __a3, 0.0_f32, Vector2::ZERO, Vector2::ONE, SpriteEffects::NONE)' % src
    elif n == 4 and types[1] == 'Rectangle':
        src = '__a2' if types[2] == 'Rectangle?' else 'Some(__a2)'
        call = 'ctx.sprite_batch.draw_rect(__t, __a1, %s, __a3, 0.0_f32, Vector2::ZERO, SpriteEffects::NONE)' % src
    elif n == 9 and types[1] == 'Vector2':
        src = '__a2' if types[2] in ('Rectangle?', 'null') else 'Some(__a2)'
        if types[2] == 'null':
            src = 'None'
        scale = '__a6' if types[6] == 'Vector2' else 'Vector2::splat(__a6)'
        rot = '__a4'
        call = 'ctx.sprite_batch.draw(__t, __a1, %s, __a3, %s, __a5, %s, __a7)' % (src, rot, scale)
        if types[4] != 'float':
            raise CsError('rotation type %s' % types[4])
    elif n == 8 and types[1] == 'Rectangle':
        src = 'None' if types[2] == 'null' else ('__a2' if types[2] == 'Rectangle?' else 'Some(__a2)')
        call = 'ctx.sprite_batch.draw_rect(__t, __a1, %s, __a3, __a4, __a5, __a6)' % src
    else:
        raise CsError('spriteBatch.Draw(%s)' % types)
    # when a destination rectangle / position argument is a null-typed literal the call text above already handles it
    code = '{ %s if let Some(__t) = __tex.as_ref() { %s; } }' % (' '.join(pre), call)
    return (code, 'void')


def tile_batch_draw(c, tb, args):
    if len(args) != 5:
        raise CsError('tileBatch.Draw with %d arguments' % len(args))
    pre = ' '.join(['let __tex = %s;' % args[0][0]] + ['let __a%d = %s;' % (i, args[i][0]) for i in range(1, 5)])
    return ('{ %s ctx.tile_batch.draw(__tex.as_ref(), __a1, __a2, __a3, __a4, 1.0_f32, SpriteEffects::NONE); }' % pre, 'void')


def lighting_get_color(c, args):
    return ('crate::pending::lighting_get_color(ctx.g, %s, %s)' % (conv(args[0][0], args[0][1], 'int'), conv(args[1][0], args[1][1], 'int')), 'Color')


def lighting_slices(kind):
    def h(c, args):
        arr = args[2][0]
        x, y = conv(args[0][0], args[0][1], 'int'), conv(args[1][0], args[1][1], 'int')
        return ('{ let mut __s = %s; crate::pending::lighting_get_color_%s_slice_colors(ctx.g, %s, %s, &mut __s); %s = __s; }' % (arr, kind, x, y, arr), 'void')
    return h


def corner_colors(c, args):
    out = args[2][0]
    name = out.split(':')[1]
    c.declare(name, 'VertexColors')
    c.emit('let mut %s: VertexColors = crate::pending::lighting_get_corner_colors_scaled(ctx.g, %s, %s, %s);' % (lname(name), conv(args[0][0], args[0][1], 'int'), conv(args[1][0], args[1][1], 'int'), (conv(args[3][0], args[3][1], 'float') if len(args) > 3 else '1.0_f32')))
    return ('', 'void')


def in_world(c, args):
    x, y = conv(args[0][0], args[0][1], 'int'), conv(args[1][0], args[1][1], 'int')
    fluff = conv(args[2][0], args[2][1], 'int') if len(args) > 2 else '0'
    return ('terraria_sim::world_gen::tile_frame::in_world(%s, %s, %s, %s)' % (G, x, y, fluff), 'bool')


def unported_value(name, val, t):
    return lambda c, a: ('unported!("%s", %s)' % (name, val), t)


def unported_stmt(name):
    return lambda c, a: ('unported!("%s")' % name, 'void')


CALLS_EXTRA = {
    'Lighting.GetColor': lighting_get_color,
    'Lighting.GetColor9Slice': lighting_slices('9'),
    'Lighting.GetColor4Slice': lighting_slices('4'),
    'Lighting.GetCornerColors': corner_colors,
    'WorldGen.InWorld': in_world,
    'SkyManager.ProcessCloudAlpha': unported_value('SkyManager.Instance.ProcessCloudAlpha', '1.0_f32', 'float'),
    'NewTileAt': unported_stmt('Main.tile[i, j] = new Tile() (unreachable: the position was checked with WorldGen.InWorld)'),
    'UnportedSky': lambda c, a: ('unported!("SkyManager.Instance.%s")' % a[0][0].strip('"'), 'void'),
    'Cloud.resetClouds': lambda c, a: ('terraria_sim::cloud::reset_clouds(ctx.g)', 'void'),
}
if MODE == 'sim':
    CALLS_EXTRA = {'WorldGen.InWorld': in_world, 'SetBackColorNoop': None}
    del CALLS_EXTRA['SetBackColorNoop']

# static fields
STATICS_EXTRA = {
    'SurfaceBackgroundIsForest': ('bool[]', 'terraria_data::sets::surface_background_id::sets::IS_FOREST'),
    'spriteBatch': ('SpriteBatch', 'ctx.sprite_batch'),
    'tileBatch': ('TileBatch', 'ctx.tile_batch'),
    'shimmerAlpha': ('float', 'ctx.state.shimmer_alpha'),
    'offScreenRange': ('int', 'ctx.state.off_screen_range'),
    'drawToScreen': ('bool', 'ctx.state.draw_to_screen'),
    'gfxQuality': ('float', 'ctx.state.gfx_quality'),
    'backgroundWidth': ('int[]', '{G}.main.background_width'),
    'backgroundHeight': ('int[]', '{G}.main.background_height'),
    'wallLight': ('bool[]', '{G}.main.wall_light'),
    'worldID': ('int', '0'),
}
STATIC_MEMBERS_EXTRA = {
    'TextureAssets.Background': ('TA:background', ''),
    'TextureAssets.Underworld': ('TA:underworld', ''),
    'TextureAssets.BlackTile': ('Texture2D', 'ctx.texture("Images/Black_Tile")'),
    'TextureAssets.MagicPixel': ('Texture2D', 'ctx.texture("Images/MagicPixel")'),
    'FocusHelper.AllowSkyMovement': ('bool', 'true'),
    'Lighting.NotRetro': ('bool', 'crate::pending::lighting_not_retro(ctx.g)' if MODE == 'draw' else 'crate::lighting::not_retro(g)'),
}
if MODE == 'sim':
    for k in ('spriteBatch', 'tileBatch', 'shimmerAlpha', 'offScreenRange', 'drawToScreen', 'gfxQuality'):
        STATICS_EXTRA.pop(k)
    STATICS_EXTRA['wallLight'] = ('bool[]', 'g.main.wall_light')

STATICS = common.make_statics(STATICS_EXTRA)
STATIC_MEMBERS = common.make_static_members(STATIC_MEMBERS_EXTRA)
for k, v in list(STATICS.items()):
    STATICS[k] = (v[0], v[1])
CALLS = common.make_calls(CALLS_EXTRA)
MEMBERS = dict(common.MEMBERS)
MEMBERS['Asset.Value'] = ('Texture2D', '{0}')
METHODS = dict(common.METHODS)
METHODS['SpriteBatch.Draw()'] = lambda c, s, a: draw_call(c, s, a)
METHODS['TileBatch.Draw()'] = tile_batch_draw
METHODS['TileBatch.Begin()'] = lambda c, s, a: ('ctx.tile_batch.begin_default()', 'void')
METHODS['TileBatch.End()'] = lambda c, s, a: ('ctx.tile_batch.end(ctx.list)', 'void')
METHODS['Rectangle.Inflate()'] = lambda c, s, a: ('%s.inflate(%s, %s)' % (s, a[0][0], a[1][0]), 'void')
METHODS['Texture2D.Frame()'] = lambda c, s, a: ('crate::main_backgrounds::texture_frame(&%s, %s)' % (s, ', '.join(conv(x[0], x[1], 'int') for x in a)), 'Rectangle')
METHODS['Tile.active()'] = lambda c, s, a: ('%s.active()' % s, 'bool')
METHODS['Tile.invisibleWall()'] = lambda c, s, a: ('%s.invisible_wall()' % s, 'bool')


def texture_code(fn, kind, idx):
    if kind == 'background':
        return 'ctx.texture(&format!("Images/Background_{}", %s))' % idx
    if kind == 'underworld':
        return 'ctx.texture(&format!("Images/Backgrounds/Underworld {}", %s))' % idx
    raise CsError('texture kind ' + kind)


def make_fn(f):
    t_file = MAIN_CS if f['cls'] == 'Main' else WORLDGEN_CS
    first, last = lines_of('Terraria.' + f['cls'], f['cs'])
    params = f['params']
    rparams = []
    for n, t, ref in params:
        if t.endswith('[]'):
            rt = '&mut [i32]' if ref else '&[i32]'
        else:
            rt = common.CS_TO_RUST.get(t) or RUST_T.get(t) or t
            if t == 'InfoToSetBackColor':
                rt = '&InfoToSetBackColor'
            if ref:
                rt = '&mut ' + rt
        rparams.append('%s%s: %s' % ('' if ref else 'mut ', lname(n), rt))
    ret = f.get('ret', 'void')
    rret = '' if ret == 'void' else ' -> ' + (RUST_T.get(ret) or ret)
    head = ('ctx: &mut DrawContext' if MODE == 'draw' else 'g: &mut Game')
    if f.get('noctx'):
        head = None
    if MODE == 'draw' and f['cs'] in ('DrawBG', 'DrawSurfaceBG'):
        pass
    sig = '%s fn %s(%s)%s' % (f['vis'], snake(f['cs']), ', '.join(([head] if head else []) + rparams), rret)
    d = dict(name=f['cs'], cs=f['cls'] + '.' + f['cs'], file=t_file, first=first, last=last, sig=sig, G=G, ret=ret,
                params=[(n, t, ref) for n, t, ref in params], pre=PRE.get(f['cs'], []))
    d['pre'] = [(r'(?m)^\s*_ = [^;]*;', ''), (r'SkyManager\.Instance\.DrawToDepth\([^;]*\);', 'UnportedSky("DrawToDepth");'), ] + d['pre']
    return d


# source-text rewrites (regex, replacement) applied before parsing, per method
SKY = r'SkyManager\.Instance\.'
PRE = {
    'DrawSurfaceBG': [(r'if \(invasionType == 4 && !SkyManager.*?Deactivate\("Martian"\);\s*\}', 'UnportedSky("Activate/Deactivate Martian");'),
                      (SKY + r'ResetDepthTracker\(\);', 'UnportedSky("ResetDepthTracker");'),
                      (SKY + r'DrawToDepth\([^;]*\);', 'UnportedSky("DrawToDepth");'),
                      (SKY + r'DrawRemainingDepth\([^;]*\);', 'UnportedSky("DrawRemainingDepth");'),
                      (SKY + r'ProcessCloudAlpha\(\)', 'SkyManager.ProcessCloudAlpha()'),
                      (r'verticalParallaxOverride \?\? ', ''),
                      (r'LatestSurfaceBackgroundBeginner = parentSpriteBatchBeginner;', ''),
                      (r'HorizonRenderer\.DrawSurfaceLayer\(num6\+\+\);', 'HorizonDrawSurfaceLayer(num6++);')],
    'DrawBG': [(r'DrawSurfaceBG\(parentSpriteBatchBeginner\)', 'DrawSurfaceBG()'),
               (r'catch \(Exception e\)\s*\{\s*if \(!ignoreErrors\)\s*\{\s*throw;\s*\}\s*TimeLogger\.DrawException\(e\);\s*\}', ''),
               (r'\btry\b', ''), (r'DrawUnderworldBackground\(GameZoomTarget\)', 'DrawUnderworldBackground(1f)')],
    'DrawBackground': [(r'TimeLogger\.StartTimestamp fromTimestamp = TimeLogger\.Start\(\);', ''), (r'TimeLogger\.DrawUndergroundBackground\.AddTime\(fromTimestamp\);', ''),
                       (r'OldDrawBackground\(\);', 'UnportedSky("OldDrawBackground");')],
    'DrawUnderworldBackground': [(SKY + r'ResetDepthTracker\(\);', 'UnportedSky("ResetDepthTracker");'), (SKY + r'DrawRemainingDepth\([^;]*\);', 'UnportedSky("DrawRemainingDepth");')],
    'DrawUnderworldBackgroundLayer': [(r'if \(!asset\.IsLoaded\)\s*\{[^}]*\}', '')],
    'DrawSurfaceBG_BackMountainsStep1': [(SKY + r'DrawToDepth\([^;]*\);', 'UnportedSky("DrawToDepth");')],
    'DrawSurfaceBG_BackMountainsStep2': [(SKY + r'DrawToDepth\([^;]*\);', 'UnportedSky("DrawToDepth");')],
    'DrawBackground_DirtBackground': [(r'tile\[num4, num5\] = new Tile\(\);', 'NewTileAt(num4, num5);'), (r'tile\[num15, num16\] = new Tile\(\);', 'NewTileAt(num15, num16);')],
}
for _n in ['Mushroom', 'Crimson', 'Snow', 'Hallow', 'GoodEvilDesert', 'Jungle', 'Desert', 'Corrupt', 'Forest']:
    PRE['DrawSurfaceBG_' + _n] = [(SKY + r'DrawToDepth\([^;]*\);', 'UnportedSky("DrawToDepth");')]
PRE['DrawBackground_DrawRockLayer'] = []
PRE['DrawBG_HandleBackgroundTransition'] = [(r'SurfaceBackgroundID\.Sets\.IsForest', 'SurfaceBackgroundIsForest')]

# ---- additions found while converting
for _i, _n in enumerate(['Full', 'ThreeQuartersAtLeft', 'HalfAtLeft', 'QuarterAtLeft', 'Empty', 'QuarterAtRight', 'HalfAtRight', 'ThreeQuartersAtRight']):
    STATIC_MEMBERS['MoonPhase.' + _n] = ('int', str(_i))
for _n, _t in [('isInGameMenuOrIsServer', 'bool'), ('CorruptionBiomeInfluence', 'float'), ('CrimsonBiomeInfluence', 'float'), ('JungleBiomeInfluence', 'float'),
               ('MushroomBiomeInfluence', 'float'), ('GraveyardInfluence', 'float'), ('BloodMoonActive', 'bool'), ('LanternNightActive', 'bool')]:
    MEMBERS['InfoToSetBackColor.' + _n] = (_t, '{0}.' + snake(_n))
MEMBERS['Texture2D.Width'] = ('int', '{0}.as_ref().map_or(0, |t| t.width)')
MEMBERS['Texture2D.Height'] = ('int', '{0}.as_ref().map_or(0, |t| t.height)')
CALLS['GetMoonPhase'] = lambda c, a: ('%s.main.moon_phase' % G, 'int')
CALLS['DontStarveSeed.ModifyNightColor'] = lambda c, a: ('unported!("DontStarveSeed.ModifyNightColor")', 'void')
CALLS['DontStarveSeed.ModifyMinimumLightColorAtNight'] = lambda c, a: ('unported!("DontStarveSeed.ModifyMinimumLightColorAtNight")', 'void')
CALLS['AuroraSky.ModifyTileColor'] = lambda c, a: ('unported!("AuroraSky.ModifyTileColor")', 'void')
CALLS['DontStarveSeed.FixBiomeDarkness'] = lambda c, a: ('crate::main_time::dont_starve_seed_fix_biome_darkness(%s, %s, &mut %s, &mut %s, &mut %s)' % ((G, a[0][0]) + tuple(x[0] for x in a[1:])), 'void')
CALLS['HorizonRenderer.ModifyHorizonLight'] = lambda c, a: ('crate::main_time::horizon_renderer_modify_horizon_light(%s, &mut %s)' % (G if MODE == 'sim' else 'ctx.g', a[0][0]), 'void')
CALLS['UnportedSky'] = lambda c, a: ('unported!("SkyManager.Instance.%s")' % a[0][0].strip('"'), 'void')

WG_STATICS = {}
for _n, _t, _i, _s in common.WORLD_FIELDS:
    WG_STATICS[_n] = (common.sig_type(_t), '{G}.world_gen.' + common.fname(_n))
WG_STATICS['hallowBG'] = ('int', '{G}.world_gen.hallow_bg')


def resolve_static(fn, name):
    if fn['name'] in ('setBG', 'SetForestBGSet', 'GetCurrentBackgroundOfStyle'):
        if name.startswith('M_'):
            return STATICS.get(name[2:])
        return WG_STATICS.get(name)
    return STATICS.get(name)


_make_fn = make_fn


def make_fn(f):  # noqa: F811
    d = _make_fn(f)
    d['keep_main'] = f['cls'] == 'WorldGen'
    return d


METHODS['rand.Next()'] = lambda c, s, a: (('%s.next(%s)' % (s, conv(a[0][0], a[0][1], 'int'))) if len(a) == 1 else ('%s.next_range(%s, %s)' % (s, conv(a[0][0], a[0][1], 'int'), conv(a[1][0], a[1][1], 'int'))), 'int')
METHODS['BackgroundVariantSet.Clear()'] = lambda c, s, a: ('%s.clear()' % s, 'void')
METHODS['BackgroundVariant.Set()'] = lambda c, s, a: ('%s.set(%s)' % (s, ', '.join(conv(x[0], x[1], 'int') for x in a)), 'void')
FUNCTIONS = [make_fn(f) for f in FUNCS]
HEADER = {
    ('draw', None): '''//! GENERATED by `tools/port_scripts/cs_bg.py` (spec `cs_bg_draw.py`, MODE=draw) from `Main.cs`: the surface and underground background drawing
//! (`DrawBG`, `DrawSurfaceBG*`, `DrawBackground*`, `DrawUnderworldBackground*`). Statements, locals and their order are the original's; the
//! hand-written helpers they call are in `main_backgrounds.rs`. Do not edit by hand: change the spec / script and regenerate.
#![allow(unused, unused_parens, unused_mut, unused_assignments, unused_comparisons, clippy::all)]

use terraria_core::xna::{Color, Rectangle, Vector2, Vector3};
use terraria_sim::graphics::VertexColors;
use terraria_sim::unported;

use crate::draw_context::DrawContext;
use crate::gfx::{SpriteEffects, Texture2D};
use crate::main_backgrounds::*;
''',
    ('sim', 'Main'): '''//! GENERATED by `tools/port_scripts/cs_bg.py` (spec `cs_bg_draw.py`, MODE=sim, FILTER=Main) from `Main.cs`: `Main.SetBackColor`,
//! `Main.GetPreferredBGStyleForPlayer`, `Main.DrawBackground_PickUndergroundBackgroundStyle`, `Main.UpdateAtmosphereTransparencyToSkyColor`.
//! Statements and locals are the original's. Do not edit by hand: change the spec / script and regenerate.
#![allow(unused, unused_parens, unused_mut, unused_assignments, unused_comparisons, clippy::all)]

use terraria_core::xna::Color;
use terraria_core::xna::Vector2;

use crate::game::Game;
use crate::main_time::InfoToSetBackColor;
use crate::unported;
''',
    ('sim', 'WorldGen'): '''//! GENERATED by `tools/port_scripts/cs_bg.py` (spec `cs_bg_draw.py`, MODE=sim, FILTER=WorldGen) from `WorldGen.cs`: `WorldGen.setBG`,
//! `WorldGen.SetForestBGSet`, `WorldGen.GetCurrentBackgroundOfStyle`. Do not edit by hand: change the spec / script and regenerate.
#![allow(unused, unused_parens, unused_mut, unused_assignments, unused_comparisons, clippy::all)]

use crate::game::Game;
''',
}[(MODE, FILTER if MODE == 'sim' else None)]

MEMBERS['Color[].Length'] = ('int', '({0}.len() as i32)')
MEMBERS['bool[].Length'] = ('int', '({0}.len() as i32)')

for _n in ['TopLeftColor', 'TopRightColor', 'BottomLeftColor', 'BottomRightColor']:
    MEMBERS['VertexColors.' + _n] = ('Color', '{0}.' + snake(_n))
METHODS['Color.ToVector3()'] = lambda c, s, a: ('%s.to_vector3()' % s, 'Vector3')
