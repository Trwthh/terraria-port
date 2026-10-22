"""Shared vocabulary of the sky / background converter (see cs_bg.py): the `Main` / `WorldGen` statics the code reads and writes
(they become fields of `MainState` / `WorldGenState` in terraria-sim), the calls it makes, and the member tables of the types it uses."""
import os
import re

from cs_bg import CsError, conv, is_num, promote, snake, lname, RUST_T

CR = 'crate' if os.environ.get('MODE', 'draw') == 'sim' else 'terraria_sim'

# --------------------------------------------------------------------------------------------------------------------
# New fields of terraria_sim::game::MainState: (C# name, C# type, initial value as in the declaration, source line)
# types: int float double bool short Color Vector2 ; arrays "int[N]" "float[N]" "Color[N]"
# --------------------------------------------------------------------------------------------------------------------
MAIN_FIELDS = [
    ('moonPhase', 'int', '0', 'Main.cs:1259'),
    ('bloodMoon', 'bool', 'false', 'Main.cs:1267'),
    ('eclipse', 'bool', 'false', 'Main.cs:1285'),
    ('moonType', 'int', '0', 'Main.cs:1407'),
    ('pumpkinMoon', 'bool', 'false', 'Main.cs:1269'),
    ('snowMoon', 'bool', 'false', 'Main.cs:1271'),
    ('dayRate', 'int', '1', 'Main.cs:749'),
    ('desiredWorldTilesUpdateRate', 'int', '1', 'Main.cs:751'),
    ('fastForwardTimeToDusk', 'bool', 'false', 'Main.cs:2318'),
    ('fastForwardTimeToDawn', 'bool', 'false', 'Main.cs:2314'),
    ('sunModY', 'short', '0', 'Main.cs:1261'),
    ('moonModY', 'short', '0', 'Main.cs:1263'),
    ('eclipseLight', 'float', '0f32', 'Main.cs:1287'),
    ('atmo', 'float', '0f32', 'Main.cs:2672'),
    ('ColorOfTheSkies', 'Color', 'terraria_core::xna::Color::default()', 'Main.cs:2678'),
    ('ColorOfSurfaceBackgroundsBase', 'Color', 'terraria_core::xna::Color::WHITE', 'Main.cs:2680'),
    ('ColorOfSurfaceBackgroundsModified', 'Color', 'terraria_core::xna::Color::WHITE', 'Main.cs:2682'),
    ('cloudAlpha', 'float', '0f32', 'Main.cs:1273'),
    ('maxRaining', 'float', '0f32', 'Main.cs:1275'),
    ('oldMaxRaining', 'float', '0f32', 'Main.cs:1277'),
    ('lightning', 'float', '0f32', 'Main.cs:2656'),
    ('windSpeedTarget', 'float', '0f32', 'Main.cs:1341'),
    ('numClouds', 'int', '200', 'Main.cs:1335'),
    ('cloudBGActive', 'float', '0f32', 'Main.cs:627'),
    ('cloudBGAlpha', 'float', '0f32', 'Main.cs:625'),
    ('cloudBGX', 'float[2]', None, 'Main.cs:623'),
    ('cloudBG', 'int[2]', '[112, 113]', 'Main.cs:629'),
    ('checkForSpawns', 'int', '0', 'Main.cs:1289'),
    ('invasionDelay', 'int', '0', 'Main.cs:1996'),
    ('invasionType', 'int', '0', 'Main.cs:1990'),
    ('tenthAnniversaryWorld', 'bool', 'false', 'Main.cs:333'),
    ('drunkWorld', 'bool', 'false', 'Main.cs:329'),
    ('notTheBeesWorld', 'bool', 'false', 'Main.cs:337'),
    ('vampireSeed', 'bool', 'false', 'Main.cs:347'),
    ('onlyShimmerOceanWorldsGeneration', 'bool', 'false', 'Main.cs (static, false outside the world generator)'),
    ('numStars', 'int', '0', 'Main.cs:1327'),
    ('resetClouds', 'bool', 'true', 'Main.cs:1357'),
    ('mapFullscreen', 'bool', 'false', 'Main.cs:917'),
    ('BackgroundEnabled', 'bool', 'true', 'Main.cs:807'),
    ('background', 'int', '0', 'Main.cs:1235'),
    ('bgStyle', 'int', '0', 'Main.cs:813'),
    ('bgDelay', 'int', '0', 'Main.cs:811'),
    ('bgAlphaFrontLayer', 'float[16]', None, 'Main.cs:815'),
    ('bgAlphaFarBackLayer', 'float[16]', None, 'Main.cs:817'),
    ('bgFrame', 'int[16]', None, 'Main.cs:819'),
    ('bgFrameCounter', 'int[16]', None, 'Main.cs:821'),
    ('instantBGTransitionCounter', 'int', '2', 'Main.cs:809'),
    ('treeMntBGSet1', 'int[2]', None, 'Main.cs:631'),
    ('treeMntBGSet2', 'int[2]', None, 'Main.cs:633'),
    ('treeMntBGSet3', 'int[2]', None, 'Main.cs:635'),
    ('treeMntBGSet4', 'int[2]', None, 'Main.cs:637'),
    ('treeBGSet1', 'int[3]', None, 'Main.cs:639'),
    ('treeBGSet2', 'int[3]', None, 'Main.cs:641'),
    ('treeBGSet3', 'int[3]', None, 'Main.cs:643'),
    ('treeBGSet4', 'int[3]', None, 'Main.cs:645'),
    ('corruptBG', 'int[3]', None, 'Main.cs:647'),
    ('jungleBG', 'int[3]', None, 'Main.cs:649'),
    ('snowMntBG', 'int[2]', None, 'Main.cs:651'),
    ('snowBG', 'int[3]', None, 'Main.cs:653'),
    ('hallowBG', 'int[3]', None, 'Main.cs:655'),
    ('crimsonBG', 'int[3]', None, 'Main.cs:657'),
    ('desertBackgroundSet', 'BackgroundVariantSet', 'crate::main_time::BackgroundVariantSet::new()', 'Main.cs:659'),
    ('mushroomBG', 'int[3]', None, 'Main.cs:661'),
    ('oceanBG', 'int', '0', 'Main.cs:663'),
    ('underworldBG', 'int[5]', None, 'Main.cs:665'),
    ('caveBackX', 'int[4]', None, 'Main.cs:671'),
    ('caveBackStyle', 'int[4]', None, 'Main.cs:673'),
    ('iceBackStyle', 'int', '0', 'Main.cs:675'),
    ('hellBackStyle', 'int', '0', 'Main.cs:677'),
    ('jungleBackStyle', 'int', '0', 'Main.cs:679'),
    ('undergroundBackground', 'int', '0', 'Main.cs:565'),
    ('oldUndergroundBackground', 'int', '0', 'Main.cs:567'),
    ('ugBackTransition', 'float', '0f32', 'Main.cs:1239'),
    ('magmaBGFrame', 'int', '0', 'Main.cs:1069'),
    ('magmaBGFrameCounter', 'int', '0', 'Main.cs:1071'),
    ('caveParallax', 'float', '0.88f32', 'Main.cs:1189'),
    ('hellBlackBoxBottom', 'float', '0f32', 'Main.cs:2632'),
    ('_drawBackground_backTexture', 'int[7]', None, 'Main.cs:2636'),
    ('_drawBackground_oldBackTexture', 'int[7]', None, 'Main.cs:2638'),
    ('_drawBackground_tempBack', 'int[7]', None, 'Main.cs:2640'),
    ('_drawbackground_colorSlices', 'Color[9]', None, 'Main.cs:2634'),
    ('bgParallax', 'double', '0.0f64', 'Main.cs:939'),
    ('bgScale', 'float', '1f32', 'Main.cs:2674'),
    ('bgTopY', 'float', '0f32', 'Main.cs:949'),
    ('bgStartX', 'int', '0', 'Main.cs:941'),
    ('bgLoops', 'int', '0', 'Main.cs:943'),
    ('bgStartY', 'int', '0', 'Main.cs:945'),
    ('bgLoopsY', 'int', '0', 'Main.cs:947'),
    ('bgWidthScaled', 'int', '1024', 'Main.cs:2676, (int)(1024f * bgScale)'),
    ('scAdj', 'float', '0f32', 'Main.cs:2684'),
    ('backgroundLayerTransitionSpeed', 'float', '0.05f32', 'Main.cs:2670'),
    ('SmoothedMushroomLightInfluence', 'float', '0f32', 'Main.cs:1359'),
    ('GraveyardVisualIntensity', 'float', '0f32', 'Main.cs:1403'),
    ('LastCelestialBodyPosition', 'Vector2', 'terraria_core::xna::Vector2::ZERO', 'Main.cs:2708'),
]

WORLD_FIELDS = [
    ('treeBG1', 'int', '0', 'WorldGen.cs'),
    ('treeBG2', 'int', '0', 'WorldGen.cs'),
    ('treeBG3', 'int', '0', 'WorldGen.cs'),
    ('treeBG4', 'int', '0', 'WorldGen.cs'),
    ('corruptBG', 'int', '0', 'WorldGen.cs:4078'),
    ('jungleBG', 'int', '0', 'WorldGen.cs:4080'),
    ('snowBG', 'int', '0', 'WorldGen.cs:4082'),
    ('crimsonBG', 'int', '0', 'WorldGen.cs:4086'),
    ('desertBG', 'int', '0', 'WorldGen.cs'),
    ('oceanBG', 'int', '0', 'WorldGen.cs:4090'),
    ('mushroomBG', 'int', '0', 'WorldGen.cs:4092'),
    ('underworldBG', 'int', '0', 'WorldGen.cs:4094'),
    ('drunkWorldGen', 'bool', 'false', 'WorldGen.cs'),
    ('remixWorldGen', 'bool', 'false', 'WorldGen.cs'),
    ('notTheBees', 'bool', 'false', 'WorldGen.cs'),
    ('BackgroundsCache', 'BackgroundChangeFlashInfo', 'crate::main_time::BackgroundChangeFlashInfo::new()', 'WorldGen.cs'),
]  # WorldGen.hallowBG already exists as `hallow_bg`

CS_TO_RUST = {'int': 'i32', 'float': 'f32', 'double': 'f64', 'bool': 'bool', 'short': 'i16', 'Color': 'terraria_core::xna::Color', 'Vector2': 'terraria_core::xna::Vector2',
              'BackgroundVariantSet': 'crate::main_time::BackgroundVariantSet', 'BackgroundChangeFlashInfo': 'crate::main_time::BackgroundChangeFlashInfo'}


def field_rust(cs_type):
    m = re.fullmatch(r'(\w+)\[(\d+)\]', cs_type)
    if m:
        return '[%s; %s]' % (CS_TO_RUST[m.group(1)], m.group(2))
    return CS_TO_RUST[cs_type]


def field_init(cs_type, init):
    if init is not None:
        return init
    m = re.fullmatch(r'(\w+)\[(\d+)\]', cs_type)
    if m:
        base = {'int': '0', 'float': '0f32', 'Color': 'terraria_core::xna::Color::default()'}[m.group(1)]
        return '[%s; %s]' % (base, m.group(2))
    raise CsError('no init for ' + cs_type)


def sig_type(cs_type):
    m = re.fullmatch(r'(\w+)\[(\d+)\]', cs_type)
    if m:
        return m.group(1) + '[]'
    return cs_type


def rust_field_blocks():
    """(MainState fields text, MainState init text, WorldGenState fields, WorldGenState init)."""
    def block(fields, indent):
        decl, init = [], []
        for name, t, i, src in fields:
            r = snake(name).lstrip('_') if name.startswith('_') else snake(name)
            decl.append('%s/// `%s` (%s), initially `%s`. Written by the sky / background code.' % (indent, name if '.' not in name else name, src, field_init(t, i) if len(field_init(t, i)) < 40 else '...'))
            decl.append('%spub %s: %s,' % (indent, r, field_rust(t)))
            init.append('%s%s: %s,' % (indent + '    ', r, field_init(t, i)))
        return '\n'.join(decl), '\n'.join(init)
    return block(MAIN_FIELDS, '    '), block(WORLD_FIELDS, '    ')


def fname(name):
    return snake(name).lstrip('_') if name.startswith('_') else snake(name)


# --------------------------------------------------------------------------------------------------------------------
# vocabulary
# --------------------------------------------------------------------------------------------------------------------

def make_statics(extra):
    st = {}
    for name, t, i, src in MAIN_FIELDS:
        st[name] = (sig_type(t), '{G}.main.' + fname(name))
    st['screenPosition'] = ('Vector2', 'Vector2::new({G}.main.screen_position.0, {G}.main.screen_position.1)')
    for cs, rs, t in [('screenWidth', 'screen_width', 'int'), ('screenHeight', 'screen_height', 'int'), ('worldSurface', 'world_surface', 'double'),
                      ('rockLayer', 'rock_layer', 'double'), ('maxTilesX', 'max_tiles_x', 'int'), ('maxTilesY', 'max_tiles_y', 'int'), ('gameMenu', 'game_menu', 'bool'),
                      ('netMode', 'net_mode', 'int'), ('time', 'time', 'double'), ('dayTime', 'day_time', 'bool'), ('remixWorld', 'remix_world', 'bool'),
                      ('dontStarveWorld', 'dont_starve_world', 'bool'), ('raining', 'raining', 'bool'), ('windSpeedCurrent', 'wind_speed_current', 'float'),
                      ('hardMode', 'hard_mode', 'bool'), ('GlobalTimeWrappedHourly', 'global_time_wrapped_hourly', 'float'), ('GameUpdateCount', 'game_update_count', 'uint'),
                      ('mouseTextColor', 'mouse_text_color', 'byte'), ('tileColor', 'tile_color', 'Color'), ('sundialCooldown', 'sundial_cooldown', 'int'),
                      ('moondialCooldown', 'moondial_cooldown', 'int'), ('dungeonY', 'dungeon_y', 'int'), ('spawnTileY', 'spawn_tile_y', 'int'), ('getGoodWorld', 'get_good_world', 'bool'),
                      ('myPlayer', 'my_player', 'int'), ('treeX', 'tree_x', 'int[]'), ('treeStyle', 'tree_style', 'int[]')]:
        st[cs] = (t, '{G}.main.' + rs)
    st['UnderworldLayer'] = ('int', '{G}.main.underworld_layer()')
    st['isThereAWorldSurface'] = ('bool', '({G}.main.world_surface > 50.0_f64)')
    st['rand'] = ('rand', '{G}.main.rand')
    st.update(extra)
    return st


def make_static_members(extra):
    m = {}
    m['Vector2.Zero'] = ('Vector2', 'Vector2::ZERO')
    m['Vector2.One'] = ('Vector2', 'Vector2::ONE')
    m['Color.White'] = ('Color', 'Color::WHITE')
    m['Color.Black'] = ('Color', 'Color::BLACK')
    m['Color.Transparent'] = ('Color', 'Color::TRANSPARENT')
    m['SpriteEffects.None'] = ('SpriteEffects', 'SpriteEffects::NONE')
    m['SpriteEffects.FlipHorizontally'] = ('SpriteEffects', 'SpriteEffects::FLIP_HORIZONTALLY')
    m['SpriteEffects.FlipVertically'] = ('SpriteEffects', 'SpriteEffects::FLIP_VERTICALLY')
    m['Lighting.GlobalBrightness'] = ('float', '{G}.lighting.global_brightness')
    m['Camera.ScaledPosition'] = ('Vector2', 'Vector2::new({G}.main.screen_position.0, {G}.main.screen_position.1)')
    for n in ['treeBG1', 'treeBG2', 'treeBG3', 'treeBG4', 'corruptBG', 'jungleBG', 'snowBG', 'crimsonBG', 'desertBG', 'oceanBG', 'mushroomBG', 'underworldBG']:
        m['WorldGen.' + n] = ('int', '{G}.world_gen.' + fname(n))
    m['WorldGen.BackgroundsCache'] = ('BackgroundChangeFlashInfo', '{G}.world_gen.backgrounds_cache')
    m['WorldGen.hallowBG'] = ('int', '{G}.world_gen.hallow_bg')
    for n in ['drunkWorldGen', 'remixWorldGen', 'notTheBees']:
        m['WorldGen.' + n] = ('bool', '{G}.world_gen.' + fname(n))
    # Main.SceneMetrics (properties of the active scene metrics)
    for n, t in [('ZoneBeach', 'bool'), ('ZoneHallow', 'bool'), ('ZoneCorrupt', 'bool'), ('ZoneCrimson', 'bool'), ('ZoneGlowshroom', 'bool'), ('ZoneDesert', 'bool'),
                 ('ZoneJungle', 'bool'), ('ZoneSnow', 'bool'), ('BloodTileCount', 'int'), ('EvilTileCount', 'int'), ('HolyTileCount', 'int'), ('HoneyBlockCount', 'int'),
                 ('SnowTileCount', 'int'), ('JungleTileCount', 'int'), ('MushroomTileCount', 'int'), ('BloodMoonMonolith', 'bool')]:
        m['SceneMetrics.' + n] = (t, '{G}.main.scene_metrics.' + fname(n))
    for n, t in [('SnowTileThreshold', 'int'), ('JungleTileThreshold', 'int'), ('HallowTileThreshold', 'int'), ('HallowTileMax', 'int'), ('CorruptionTileMax', 'int'),
                 ('CrimsonTileMax', 'int'), ('JungleTileMax', 'int'), ('MushroomTileMax', 'int')]:
        m['SceneMetrics.' + n] = (t, CR + '::scene_metrics::SceneMetrics::x')
    m['SceneMetrics.SnowTileThreshold'] = ('int', CR + '::scene_metrics::snow_tile_threshold({G})')
    m['SceneMetrics.JungleTileThreshold'] = ('int', CR + '::scene_metrics::JUNGLE_TILE_THRESHOLD')
    m['SceneMetrics.HallowTileThreshold'] = ('int', CR + '::scene_metrics::HALLOW_TILE_THRESHOLD')
    m['SceneMetrics.HallowTileMax'] = ('int', CR + '::scene_metrics::HALLOW_TILE_MAX')
    m['SceneMetrics.CorruptionTileMax'] = ('int', CR + '::scene_metrics::CORRUPTION_TILE_MAX')
    m['SceneMetrics.CrimsonTileMax'] = ('int', CR + '::scene_metrics::CRIMSON_TILE_MAX')
    m['SceneMetrics.JungleTileMax'] = ('int', CR + '::scene_metrics::JUNGLE_TILE_MAX')
    m['SceneMetrics.MushroomTileMax'] = ('int', CR + '::scene_metrics::MUSHROOM_TILE_MAX')
    m.update(extra)
    return m


def _a(args, i, t):
    return conv(args[i][0], args[i][1], t)


def call_math_minmax(fn):
    def h(c, args):
        t = promote(args[0][1], args[1][1])
        return ('(%s).%s(%s)' % (_a(args, 0, t), fn, _a(args, 1, t)), t)
    return h


def make_calls(extra):
    C = {}
    C['Math.Max'] = call_math_minmax('max')
    C['Math.Min'] = call_math_minmax('min')
    C['Math.Abs'] = lambda c, a: ('(%s).abs()' % a[0][0], a[0][1])
    C['Math.Floor'] = lambda c, a: ('(%s).floor()' % _a(a, 0, 'double'), 'double')
    C['Math.Ceiling'] = lambda c, a: ('(%s).ceil()' % _a(a, 0, 'double'), 'double')
    C['Math.Round'] = lambda c, a: ('(%s).round_ties_even()' % _a(a, 0, 'double'), 'double')
    C['Math.Pow'] = lambda c, a: ('(%s).powf(%s)' % (_a(a, 0, 'double'), _a(a, 1, 'double')), 'double')
    C['Math.Sin'] = lambda c, a: ('(%s).sin()' % _a(a, 0, 'double'), 'double')
    C['Math.Cos'] = lambda c, a: ('(%s).cos()' % _a(a, 0, 'double'), 'double')
    C['Math.IEEERemainder'] = lambda c, a: (CR + '::main_time::ieee_remainder(%s, %s)' % (_a(a, 0, 'double'), _a(a, 1, 'double')), 'double')
    C['MathHelper.Clamp'] = lambda c, a: ('terraria_core::xna::math_helper::clamp(%s, %s, %s)' % (_a(a, 0, 'float'), _a(a, 1, 'float'), _a(a, 2, 'float')), 'float')
    C['MathHelper.Lerp'] = lambda c, a: ('terraria_core::xna::math_helper::lerp(%s, %s, %s)' % (_a(a, 0, 'float'), _a(a, 1, 'float'), _a(a, 2, 'float')), 'float')
    C['Utils.Clamp'] = lambda c, a: ('(%s).clamp(%s, %s)' % (a[0][0], a[1][0], a[2][0]), a[0][1])
    C['Color.Lerp'] = lambda c, a: ('Color::lerp(%s, %s, %s)' % (a[0][0], a[1][0], _a(a, 2, 'float')), 'Color')

    def array_clear(c, a):
        return ('%s.fill(Default::default())' % a[0][0], 'void')
    C['Array.Clear'] = array_clear
    C.update(extra)
    return C


MEMBERS = {
    'Color.R': ('byte', '{0}.r()'), 'Color.G': ('byte', '{0}.g()'), 'Color.B': ('byte', '{0}.b()'), 'Color.A': ('byte', '{0}.a()'),
    'Vector2.X': ('float', '{0}.x'), 'Vector2.Y': ('float', '{0}.y'),
    'Vector3.X': ('float', '{0}.x'), 'Vector3.Y': ('float', '{0}.y'), 'Vector3.Z': ('float', '{0}.z'),
    'Rectangle.Width': ('int', '{0}.width'), 'Rectangle.Height': ('int', '{0}.height'), 'Rectangle.X': ('int', '{0}.x'), 'Rectangle.Y': ('int', '{0}.y'),
    'int[].Length': ('int', '({0}.len() as i32)'), 'float[].Length': ('int', '({0}.len() as i32)'),
    'BackgroundVariantSet.Pure': ('BackgroundVariant', '{0}.pure'), 'BackgroundVariantSet.Corrupt': ('BackgroundVariant', '{0}.corrupt'),
    'BackgroundVariantSet.Crimson': ('BackgroundVariant', '{0}.crimson'), 'BackgroundVariantSet.Hallow': ('BackgroundVariant', '{0}.hallow'),
    'BackgroundVariant.Backgrounds': ('int[]', '{0}.backgrounds'), 'BackgroundVariant.HasAny': ('bool', '{0}.has_any()'),
    'Texture2D.Value': ('Texture2D', '{0}'),
}

METHODS = {
    'BackgroundChangeFlashInfo.GetFlashPower()': lambda c, s, a: ('%s.get_flash_power(%s)' % (s, conv(a[0][0], a[0][1], 'int')), 'float'),
    'Vector2.Floor()': lambda c, s, a: ('Vector2::new((%s).x.floor(), (%s).y.floor())' % (s, s), 'Vector2'),
    'Texture2D.Width()': lambda c, s, a: ('%s.as_ref().map_or(0, |t| t.width)' % s, 'int'),
    'Texture2D.Height()': lambda c, s, a: ('%s.as_ref().map_or(0, |t| t.height)' % s, 'int'),
}
