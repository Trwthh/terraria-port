// AudioRec: what the ORIGINAL client asks of the audio engine, call by call (the audio counterpart of
// xna\Graphics\Rec.cs; port side: port\crates\terraria-host\src\audio\trace.rs writes the same lines).
//
// Where it records: the boundary between XNA's managed audio classes and their native engine. XNA 4's
// SoundEffect / SoundEffectInstance / DynamicSoundEffectInstance (Microsoft.Xna.Framework.dll) do their argument
// checks and state bookkeeping in managed code and call `SoundEffectUnsafeNativeMethods.*` for everything that reaches
// XAudio2; the XACT classes (Microsoft.Xna.Framework.Xact.dll: AudioEngine, SoundBank, WaveBank, Cue, AudioCategory)
// call `UnsafeNativeMethods.{Engine, SoundBank, WaveBank, Cue}.*` for everything that reaches xactengine3_7. Those
// static methods are replaced (Detour.cs) by the methods below: XNA's own managed code runs as it is (range checks,
// exceptions, `isPacketSubmitted`, the State mapping, Cue's IsPlaying / IsStopped bits), the engine behind it is a
// deterministic model (no XAudio2 voice, no XACT engine, nothing reaches the speakers), and every call is one line.
//
// Derived XAudio2 parameters come from XNA's REAL kernel code: SetVolume / SetPitch / SetPan of a sound effect
// instance run the kernel functions of the mixed-mode assembly (<Module>.Microsoft.Xna.Framework.Audio.
// KernelSoundEffectInstance.SetVolume / SetPitch / SetPan: the pan law, the frequency ratio) on a stand-in kernel object
// whose IXAudio2SourceVoice is a fake COM object that records what the kernel hands to XAudio2: the voice volume, the
// output matrix (source channels x 2 destination channels), the frequency ratio. Those values are written on the line.
//
// The engine model (port: terraria-host audio.rs, the same rules; pinned, like the clock):
//   - a sound effect instance has the kernel's voice states (Playing 1, Stopping 2, Stopped 4, Paused 8; Play from Stopped
//     resubmits the buffer when none is queued; Stop(immediate) with a queued buffer is Stopping until the next frame,
//     then Stopped and the buffer flushed; Pause of a playing voice is Paused)
//   - time: every frame (Advance, before the frame's Update, as the port's begin_frame) a playing voice consumes
//     ticks * frequency ratio of source time; a non-looped buffer (loop count 0) ends when its frames are consumed:
//     consumed * sampleRate >= frames * 10^7 * 2^24 with consumed = sum of (ratio * 2^24) * ticks (exact integers)
//   - frames of a buffer: PCM data bytes / block align; MS ADPCM (data bytes / block align) * samples per block
//   - XACT: a cue from GetCue is Prepared (status 4); Play: Playing (8); Pause(1) of a playing cue: Paused (0x40);
//     Pause(0) of a paused cue: Playing; Stop (either option): Stopped (0x20); a cue does not end by itself in the run
//     (the port's test checks that none of its cues ran out); a wave bank is Prepared (4) at once; every cue name exists
//   - the device: 2 output channels (XACT destination channels 2); the mastering voice is not created
//
// Lines (fields separated by one space; float = 8 hex digits of its bits; FNV-1a 64 as 16 hex digits):
//   se.create <label> <formatBytes>:<fnv> <dataBytes>:<fnv> <frames>      CreateSoundEffectHandle (label: content asset
//                                                                         below Content/, or "stream" for FromStream)
//   se.release <label> | se.master <f> | se.getmaster
//   si.create si<k> <label> | si.destroy si<k>                            (k: creation order from 0)
//   si.volume si<k> <f> v=<f>                                             v: XAudio2 voice volume (kernel)
//   si.pitch si<k> <f> r=<f>                                              r: frequency ratio (kernel)
//   si.pan si<k> <f> m=<f>,<f>[,<f>,<f>]                                  m: output matrix, [dst * src + s] (kernel)
//   si.submit si<k> <loopStart> <loopLength> <loopCount>
//   si.play si<k> <state after> | si.stop si<k> <0|1> <state after> | si.pause si<k> <state after>
//   si.state si<k> <state>
//   sd.create si<k> <formatBytes>:<fnv> | sd.submit si<k> <bytes>:<fnv> | sd.clear si<k> | sd.pending si<k> <n>
//   xe.create <file> <lookahead> <renderer> | xe.update | xe.release | xe.renderers
//   xe.category <name> <index> | xe.catvolume <index> <f> | xe.catpause <index> <0|1> | xe.catstop <index> <opt>
//   xe.getglobal <name> | xe.setglobal <name> <f>
//   xs.create <file> | xs.status <bits> | xs.getcue <name> xc<k> | xs.playcue <name> | xs.release
//   xw.create <file> | xw.stream <file> <offset> <packetsize> | xw.status <bits> | xw.release
//   xc.play xc<k> | xc.stop xc<k> <opt> | xc.pause xc<k> <0|1> | xc.setvar xc<k> <name> <f> | xc.getvar xc<k> <name>
//   xc.status xc<k> <bits> | xc.release xc<k>
// Only calls on the game's main thread are recorded (finalizers release cues on their own thread).
using System;
using System.Collections.Generic;
using System.IO;
using System.Numerics;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static unsafe class AudioRec
{
    // ------------------------------------------------------------------------------------------------ recording
    public static TextWriter Out;
    public static bool Active, Full;
    public static ulong FrameHash = Basis;
    public static long FrameLines, Lines;
    const ulong Basis = 14695981039346656037UL, Prime = 1099511628211UL;
    static bool OnMain { get { return Thread.CurrentThread.ManagedThreadId == Microsoft.Xna.Framework.XnaStandIn.Rec.MainThread; } }

    static void L(string s)
    {
        if (!Active || !OnMain) return;
        ulong h = FrameHash;
        for (int i = 0; i < s.Length; i++) { h ^= (byte)s[i]; h *= Prime; }
        h ^= 10; h *= Prime;
        FrameHash = h;
        FrameLines++;
        if (!Full) return;
        Out.Write(s);
        Out.Write('\n');
        Lines++;
    }
    public static void FrameStart() { FrameHash = Basis; FrameLines = 0; }

    static string F(float f) { return ((uint)BitConverter.ToInt32(BitConverter.GetBytes(f), 0)).ToString("x8"); }
    static ulong Fnv(byte* p, uint n)
    {
        ulong h = Basis;
        for (uint i = 0; i < n; i++) { h ^= p[i]; h *= Prime; }
        return h;
    }
    static string Bytes(byte* p, uint n) { return n + ":" + Fnv(p, n).ToString("x16"); }
    static string FileLabel(string path) { return Microsoft.Xna.Framework.XnaStandIn.Rec.AssetLabel(path); }

    // ------------------------------------------------------------------------------------------------ the model
    class Effect { public string Label; public int Channels, Rate; public long Frames; }
    class Voice
    {
        public int K; public Effect Fx; public int State = 4; public bool Queued; public BigInteger Consumed;
        public int LoopCount; public long Ratio24 = 1L << 24; public bool Dynamic; public int Channels = 1, Rate;
        public readonly Queue<long> Buffers = new Queue<long>();
    }
    class CueM { public int K; public uint Bits = 4; public readonly Dictionary<string, float> Vars = new Dictionary<string, float>(); }
    static readonly object Sync = new object();
    static readonly Dictionary<uint, Effect> Effects = new Dictionary<uint, Effect>();
    static readonly Dictionary<uint, Voice> Voices = new Dictionary<uint, Voice>();
    static readonly Dictionary<uint, CueM> Cues = new Dictionary<uint, CueM>();
    static uint NextHandle = 1;
    static int NextVoice, NextCue;
    static float MasterVolume = 1f;
    static readonly BigInteger TicksScale = new BigInteger(10000000L) * (1L << 24);

    /// <summary>Frames a buffer of `dataLen` bytes plays (XAudio2: whole blocks).</summary>
    static long FramesOf(byte* format, uint formatSize, uint dataLen, out int channels, out int rate)
    {
        int tag = *(ushort*)format;
        channels = *(ushort*)(format + 2);
        rate = *(int*)(format + 4);
        int align = *(ushort*)(format + 12);
        if (align <= 0) return 0;
        if (tag == 2 && formatSize >= 20) return (long)(dataLen / (uint)align) * *(ushort*)(format + 18);
        return dataLen / (uint)align;
    }

    /// <summary>One frame of time (`ticks` of 100 ns) for every voice (port: terraria_host::audio::advance).</summary>
    public static void Advance(long ticks)
    {
        lock (Sync)
        {
            foreach (Voice v in Voices.Values)
            {
                if (v.State == 2) { v.State = 4; v.Queued = false; v.Consumed = BigInteger.Zero; v.Buffers.Clear(); continue; }
                if (v.State != 1) continue;
                // Consumed: source frames played so far, times 10^7 * 2^24 (sum of ratio * 2^24 * ticks * sample rate)
                if (v.Dynamic)
                {
                    // queued buffers are consumed in order; an empty queue starves (the voice keeps Playing)
                    if (v.Buffers.Count == 0) continue;
                    v.Consumed += new BigInteger(v.Ratio24) * ticks * v.Rate;
                    while (v.Buffers.Count > 0 && v.Consumed >= new BigInteger(v.Buffers.Peek()) * TicksScale)
                        v.Consumed -= new BigInteger(v.Buffers.Dequeue()) * TicksScale;
                    if (v.Buffers.Count == 0) v.Consumed = BigInteger.Zero;
                    continue;
                }
                if (!v.Queued || v.LoopCount != 0 || v.Fx == null) continue;
                v.Consumed += new BigInteger(v.Ratio24) * ticks * v.Fx.Rate;
                if (v.Consumed >= new BigInteger(v.Fx.Frames) * TicksScale) { v.State = 4; v.Queued = false; v.Consumed = BigInteger.Zero; }
            }
        }
    }

    // ------------------------------------------------------------------------------------------------ the real kernel
    // A stand-in KernelSoundEffectInstance (60 bytes: [0] vtable, [4] handle, [8] sound effect, [12] ratio, [16] the
    // IXAudio2SourceVoice, ..., [56] state) whose vtable answers GetSourceChannelCount (+80) / GetMatrixCoefficientArray
    // (+84), and whose voice records SetVolume (+48), SetOutputMatrix (+64), SetFrequencyRatio (+104).
    [UnmanagedFunctionPointer(CallingConvention.ThisCall)] delegate int ThisInt(IntPtr self);
    [UnmanagedFunctionPointer(CallingConvention.ThisCall)] delegate IntPtr ThisPtr(IntPtr self);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int VSetVolume(IntPtr self, float v, uint opset);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int VSetMatrix(IntPtr self, IntPtr dest, uint src, uint dst, IntPtr m, uint opset);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int VSetRatio(IntPtr self, float r, uint opset);
    static IntPtr KInst, KVtbl, KVoice, KVoiceVtbl, KMatrix;
    static int KChannels;
    static float KVolume, KRatio;
    static float[] KMatrixOut = new float[4];
    static uint KMatrixSrc, KMatrixDst;
    static readonly List<Delegate> KeepAlive = new List<Delegate>();
    static MethodInfo KSetVolume, KSetPitch, KSetPan;
    static Type KPtrType;

    static IntPtr Fp(Delegate d) { KeepAlive.Add(d); return Marshal.GetFunctionPointerForDelegate(d); }

    static void KernelInit(Assembly fw)
    {
        foreach (MethodInfo m in fw.ManifestModule.GetMethods(BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic))
        {
            if (m.Name.EndsWith("KernelSoundEffectInstance.SetVolume")) KSetVolume = m;
            else if (m.Name.EndsWith("KernelSoundEffectInstance.SetPitch")) KSetPitch = m;
            else if (m.Name.EndsWith("KernelSoundEffectInstance.SetPan")) KSetPan = m;
        }
        if (KSetVolume == null || KSetPitch == null || KSetPan == null) throw new Exception("the XNA audio kernel functions were not found");
        KPtrType = KSetPan.GetParameters()[0].ParameterType;
        KInst = Marshal.AllocHGlobal(64); KVtbl = Marshal.AllocHGlobal(128); KVoice = Marshal.AllocHGlobal(16); KVoiceVtbl = Marshal.AllocHGlobal(128); KMatrix = Marshal.AllocHGlobal(64 * 4);
        for (int i = 0; i < 64; i += 4) Marshal.WriteInt32(KInst, i, 0);
        for (int i = 0; i < 128; i += 4) { Marshal.WriteInt32(KVtbl, i, 0); Marshal.WriteInt32(KVoiceVtbl, i, 0); }
        Marshal.WriteIntPtr(KVtbl, 80, Fp(new ThisInt(self => KChannels)));
        Marshal.WriteIntPtr(KVtbl, 84, Fp(new ThisPtr(self => KMatrix)));
        Marshal.WriteIntPtr(KVoiceVtbl, 48, Fp(new VSetVolume((self, v, op) => { KVolume = v; return 0; })));
        Marshal.WriteIntPtr(KVoiceVtbl, 64, Fp(new VSetMatrix((self, dest, src, dst, m, op) =>
        {
            KMatrixSrc = src; KMatrixDst = dst;
            for (int i = 0; i < 4; i++) KMatrixOut[i] = ((float*)m)[i];
            return 0;
        })));
        Marshal.WriteIntPtr(KVoiceVtbl, 104, Fp(new VSetRatio((self, r, op) => { KRatio = r; return 0; })));
        Marshal.WriteIntPtr(KInst, 0, KVtbl);
        Marshal.WriteIntPtr(KVoice, 0, KVoiceVtbl);
        Marshal.WriteIntPtr(KInst, 16, KVoice);
        Marshal.WriteInt32(KInst, 56, 4);
    }
    static int KCall(MethodInfo m, float value)
    {
        object r = m.Invoke(null, new object[] { Pointer.Box((void*)KInst, KPtrType), value });
        return (int)r;
    }

    // ------------------------------------------------------------------------------------------------ SoundEffectUnsafeNativeMethods
    static int ShutdownAudioEngine() { L("se.shutdown"); return 0; }

    static int CreateSoundEffectHandle(byte* format, uint formatSize, byte* data, uint dataLength, uint* handle)
    {
        if (handle == null) return -2147024809;
        string label = "stream";
        foreach (var f in new System.Diagnostics.StackTrace(false).GetFrames())
        {
            MethodBase m = f.GetMethod();
            if (m != null && m.DeclaringType != null && m.DeclaringType.FullName == "Microsoft.Xna.Framework.Audio.SoundEffect" && m.Name == "FromStream") { label = "stream"; break; }
            if (m != null && m.DeclaringType != null && m.DeclaringType.FullName == "Microsoft.Xna.Framework.Content.SoundEffectReader") { label = Microsoft.Xna.Framework.XnaStandIn.Rec.CurrentAsset ?? "content?"; break; }
        }
        int ch, rate;
        long frames = FramesOf(format, formatSize, dataLength, out ch, out rate);
        uint h;
        lock (Sync) { h = NextHandle++; Effects[h] = new Effect { Label = label, Channels = ch, Rate = rate, Frames = frames }; }
        *handle = h;
        L("se.create " + label + " " + Bytes(format, formatSize) + " " + Bytes(data, dataLength) + " " + frames);
        return 0;
    }
    static int ReleaseSoundEffectHandle(uint h)
    {
        Effect e;
        lock (Sync) { if (!Effects.TryGetValue(h, out e)) return -2147024809; Effects.Remove(h); }
        L("se.release " + e.Label);
        return 0;
    }
    static int SetMasterVolume(float v) { MasterVolume = v; L("se.master " + F(v)); return 0; }
    static int GetMasterVolume(float* v) { if (v == null) return -2147024809; *v = MasterVolume; L("se.getmaster"); return 0; }

    static int CreateSoundEffectInstance(uint seHandle, uint* handle)
    {
        Effect e;
        Voice v;
        uint h;
        lock (Sync)
        {
            if (!Effects.TryGetValue(seHandle, out e)) return -2147024809;
            if (handle == null) return -2147024809;
            h = NextHandle++;
            v = new Voice { K = NextVoice++, Fx = e, Channels = e.Channels, Rate = e.Rate };
            Voices[h] = v;
        }
        *handle = h;
        L("si.create si" + v.K + " " + e.Label);
        return 0;
    }
    static Voice V(uint h) { Voice v; lock (Sync) Voices.TryGetValue(h, out v); return v; }
    static int DestroySoundEffectInstance(uint h)
    {
        Voice v;
        lock (Sync) { if (!Voices.TryGetValue(h, out v)) return -2147024809; Voices.Remove(h); }
        L("si.destroy si" + v.K);
        return 0;
    }
    static int SubmitPacket(uint h, int loopStart, int loopLength, int loopCount)
    {
        Voice v = V(h);
        if (v == null) return -2147024809;
        lock (Sync)
        {
            // KernelSoundEffectInstance.SubmitPacket: loop count -1 is XAUDIO2_LOOP_INFINITE (255)
            v.LoopCount = loopCount == -1 ? 255 : loopCount;
            v.Queued = true;
            v.Consumed = BigInteger.Zero;
        }
        L("si.submit si" + v.K + " " + loopStart + " " + loopLength + " " + loopCount);
        return 0;
    }
    static int Play(uint h)
    {
        Voice v = V(h);
        if (v == null) return -2147024809;
        lock (Sync)
        {
            if (v.State == 4 || v.State == 8)
            {
                if (v.State == 4 && !v.Queued && !v.Dynamic) { v.Queued = true; v.Consumed = BigInteger.Zero; }
                v.State = 1;
            }
        }
        L("si.play si" + v.K + " " + v.State);
        return 0;
    }
    static int Stop(uint h, bool immediate)
    {
        Voice v = V(h);
        if (v == null) return -2147024809;
        lock (Sync)
        {
            if (immediate)
            {
                bool queued = v.Dynamic ? v.Buffers.Count > 0 : v.Queued;
                if (!queued) v.State = 4;
                else v.State = 2;
            }
            else if (v.LoopCount != 0) { v.LoopCount = 0; v.Consumed = BigInteger.Zero; }
        }
        L("si.stop si" + v.K + " " + (immediate ? 1 : 0) + " " + v.State);
        return 0;
    }
    static int Pause(uint h)
    {
        Voice v = V(h);
        if (v == null) return -2147024809;
        lock (Sync) { if (v.State == 1) v.State = 8; }
        L("si.pause si" + v.K + " " + v.State);
        return 0;
    }
    static int SetVolume(uint h, float volume)
    {
        Voice v = V(h);
        if (v == null) return -2147024809;
        int r;
        lock (Sync) { KVolume = float.NaN; r = KCall(KSetVolume, volume); }
        L("si.volume si" + v.K + " " + F(volume) + " v=" + F(KVolume));
        return r;
    }
    static int SetPitch(uint h, float pitch)
    {
        Voice v = V(h);
        if (v == null) return -2147024809;
        int r;
        lock (Sync)
        {
            KRatio = float.NaN;
            r = KCall(KSetPitch, pitch);
            if (r >= 0) v.Ratio24 = (long)((double)KRatio * (1L << 24));
        }
        L("si.pitch si" + v.K + " " + F(pitch) + " r=" + F(KRatio));
        return r;
    }
    static int SetPan(uint h, float pan)
    {
        Voice v = V(h);
        if (v == null) return -2147024809;
        int r;
        string m;
        lock (Sync)
        {
            KChannels = v.Channels;
            for (int i = 0; i < 4; i++) KMatrixOut[i] = float.NaN;
            r = KCall(KSetPan, pan);
            m = v.Channels == 2 ? F(KMatrixOut[0]) + "," + F(KMatrixOut[1]) + "," + F(KMatrixOut[2]) + "," + F(KMatrixOut[3]) : F(KMatrixOut[0]) + "," + F(KMatrixOut[1]);
        }
        L("si.pan si" + v.K + " " + F(pan) + " m=" + m);
        return r;
    }
    static int GetState(uint h, int* state)
    {
        Voice v = V(h);
        if (v == null || state == null) return -2147024809;
        *state = v.State;
        L("si.state si" + v.K + " " + v.State);
        return 0;
    }
    static int CreateDynamicSoundEffectInstance(byte* format, uint formatSize, uint* handle)
    {
        if (handle == null) return -2147024809;
        uint h;
        Voice v;
        lock (Sync)
        {
            h = NextHandle++;
            v = new Voice { K = NextVoice++, Dynamic = true, Channels = *(ushort*)(format + 2), Rate = *(int*)(format + 4) };
            Voices[h] = v;
        }
        *handle = h;
        L("sd.create si" + v.K + " " + Bytes(format, formatSize));
        return 0;
    }
    static int SubmitBuffer(uint h, byte* data, uint size)
    {
        Voice v = V(h);
        if (v == null) return -2147024809;
        lock (Sync) v.Buffers.Enqueue(size / (uint)(2 * Math.Max(1, v.Channels)));
        L("sd.submit si" + v.K + " " + Bytes(data, size));
        return 0;
    }
    static int ClearPendingBuffers(uint h)
    {
        Voice v = V(h);
        if (v == null) return -2147024809;
        lock (Sync) { v.Buffers.Clear(); v.Consumed = BigInteger.Zero; }
        L("sd.clear si" + v.K);
        return 0;
    }
    static int GetPendingBufferCount(uint h, int* count)
    {
        Voice v = V(h);
        if (v == null || count == null) return -2147024809;
        int n;
        lock (Sync) n = v.Buffers.Count;
        *count = n;
        L("sd.pending si" + v.K + " " + n);
        return 0;
    }

    // ------------------------------------------------------------------------------------------------ XACT
    const uint E_INVALIDARG = 0x80070057u;
    static uint XEngine(string path, int length, uint lookahead, string renderer, out int destChannels, out IntPtr cleanup, out IntPtr p3d)
    {
        destChannels = 2; cleanup = IntPtr.Zero; p3d = IntPtr.Zero;
        uint h; lock (Sync) h = NextHandle++;
        L("xe.create " + FileLabel(path) + " " + lookahead + " " + (string.IsNullOrEmpty(renderer) ? "-" : renderer));
        return h;
    }
    static void XEngineUpdate(uint h) { L("xe.update"); }
    static void XEngineRelease(uint h, IntPtr a, IntPtr b, IntPtr c) { L("xe.release"); }
    static int XEngineRendererCount(uint h) { L("xe.renderers"); return 0; }
    static uint XEngineRendererDetails(uint h, int index, ref string name, ref string guid) { return E_INVALIDARG; }
    static readonly List<string> Categories = new List<string>();
    static ushort XGetCategory(uint h, string name, int length)
    {
        int i;
        lock (Sync) { i = Categories.IndexOf(name); if (i < 0) { Categories.Add(name); i = Categories.Count - 1; } }
        L("xe.category " + name + " " + i);
        return (ushort)i;
    }
    static uint XCatVolume(uint h, ushort cat, float v) { L("xe.catvolume " + cat + " " + F(v)); return 0; }
    static uint XCatPause(uint h, ushort cat, int pause) { L("xe.catpause " + cat + " " + pause); return 0; }
    static uint XCatStop(uint h, ushort cat, uint options) { L("xe.catstop " + cat + " " + options); return 0; }
    static readonly Dictionary<string, float> Globals = new Dictionary<string, float>();
    static uint XGetGlobal(uint h, string name, int length, out float v) { lock (Sync) Globals.TryGetValue(name, out v); L("xe.getglobal " + name); return 0; }
    static uint XSetGlobal(uint h, string name, int length, float v) { lock (Sync) Globals[name] = v; L("xe.setglobal " + name + " " + F(v)); return 0; }

    static uint XWaveBank(uint hEngine, string path, int length, out IntPtr cleanup) { cleanup = IntPtr.Zero; uint h; lock (Sync) h = NextHandle++; L("xw.create " + FileLabel(path)); return h; }
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern uint GetFinalPathNameByHandle(IntPtr file, StringBuilder path, uint size, uint flags);
    static uint XWaveBankStreaming(uint hEngine, IntPtr file, uint offset, short packetsize)
    {
        var sb = new StringBuilder(1024);
        GetFinalPathNameByHandle(file, sb, 1024, 0);
        uint h; lock (Sync) h = NextHandle++;
        L("xw.stream " + FileLabel(sb.ToString()) + " " + offset + " " + packetsize);
        return h;
    }
    static uint XWaveBankStatus(uint h) { L("xw.status 4"); return 4; }
    static void XWaveBankRelease(uint h, IntPtr cleanup) { L("xw.release"); }

    static uint XSoundBank(uint hEngine, string path, int length, out IntPtr cleanup) { cleanup = IntPtr.Zero; uint h; lock (Sync) h = NextHandle++; L("xs.create " + FileLabel(path)); return h; }
    static uint XSoundBankStatus(uint h) { L("xs.status 0"); return 0; }
    static void XSoundBankRelease(uint h, IntPtr cleanup) { L("xs.release"); }
    static uint XGetCue(uint hSoundBank, string name, int length, out uint hCue)
    {
        CueM c;
        lock (Sync) { hCue = NextHandle++; c = new CueM { K = NextCue++ }; Cues[hCue] = c; }
        L("xs.getcue " + name + " xc" + c.K);
        return 0;
    }
    static uint XPlayCue(uint hSoundBank, string name, int length) { L("xs.playcue " + name); return 0; }
    static CueM C(uint h) { CueM c; lock (Sync) Cues.TryGetValue(h, out c); return c; }
    static uint XCueStatus(uint h)
    {
        CueM c = C(h);
        uint bits = c == null ? 0u : c.Bits;
        if (c != null) L("xc.status xc" + c.K + " " + bits);
        return bits;
    }
    static uint XCuePlay(uint h)
    {
        CueM c = C(h);
        if (c == null) return E_INVALIDARG;
        bool ok;
        lock (Sync) { ok = c.Bits == 4; if (ok) c.Bits = 8; }
        L("xc.play xc" + c.K);
        // a cue plays once (XACTENGINE_E_INVALIDUSAGE otherwise; Cue.Play throws)
        return ok ? 0u : 0x8AC70002u;
    }
    static uint XCuePause(uint h, int pause)
    {
        CueM c = C(h);
        if (c == null) return E_INVALIDARG;
        lock (Sync)
        {
            if (pause != 0 && c.Bits == 8) c.Bits = 0x40;
            else if (pause == 0 && c.Bits == 0x40) c.Bits = 8;
        }
        L("xc.pause xc" + c.K + " " + pause);
        return 0;
    }
    static uint XCueStop(uint h, uint options)
    {
        CueM c = C(h);
        if (c == null) return E_INVALIDARG;
        lock (Sync) c.Bits = 0x20;
        L("xc.stop xc" + c.K + " " + options);
        return 0;
    }
    static uint XCueSetVariable(uint h, string name, int length, float value)
    {
        CueM c = C(h);
        if (c == null) return E_INVALIDARG;
        lock (Sync) c.Vars[name] = value;
        L("xc.setvar xc" + c.K + " " + name + " " + F(value));
        return 0;
    }
    static uint XCueGetVariable(uint h, string name, int length, out float value)
    {
        value = 0f;
        CueM c = C(h);
        if (c == null) return E_INVALIDARG;
        lock (Sync) c.Vars.TryGetValue(name, out value);
        L("xc.getvar xc" + c.K + " " + name);
        return 0;
    }
    static void XCueRelease(uint h)
    {
        CueM c;
        lock (Sync) { if (!Cues.TryGetValue(h, out c)) return; Cues.Remove(h); }
        L("xc.release xc" + c.K);
    }

    // ------------------------------------------------------------------------------------------------ installation
    static MethodInfo Mine(string name) { return typeof(AudioRec).GetMethod(name, BindingFlags.Static | BindingFlags.NonPublic); }
    static MethodInfo Find(Type t, string name, int args, string firstExtra = null)
    {
        foreach (MethodInfo m in t.GetMethods(BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic))
        {
            if (m.Name != name || m.GetParameters().Length != args) continue;
            if (firstExtra != null && m.GetParameters()[1].ParameterType.Name != firstExtra) continue;
            return m;
        }
        throw new Exception("no " + t.FullName + "." + name + "/" + args);
    }

    /// <summary>Replaces XNA's native audio layer (both assemblies) with the model above. `xact`: Microsoft.Xna.Framework.Xact.</summary>
    public static void Install(Assembly fw, Assembly xact)
    {
        KernelInit(fw);
        Type n = fw.GetType("Microsoft.Xna.Framework.Audio.SoundEffectUnsafeNativeMethods", true);
        Detour.Install(Find(n, "ShutdownAudioEngine", 0), Mine("ShutdownAudioEngine"));
        Detour.Install(Find(n, "CreateSoundEffectHandle", 5), Mine("CreateSoundEffectHandle"));
        Detour.Install(Find(n, "ReleaseSoundEffectHandle", 1), Mine("ReleaseSoundEffectHandle"));
        Detour.Install(Find(n, "SetMasterVolume", 1), Mine("SetMasterVolume"));
        Detour.Install(Find(n, "GetMasterVolume", 1), Mine("GetMasterVolume"));
        Detour.Install(Find(n, "CreateSoundEffectInstance", 2), Mine("CreateSoundEffectInstance"));
        Detour.Install(Find(n, "DestroySoundEffectInstance", 1), Mine("DestroySoundEffectInstance"));
        Detour.Install(Find(n, "SubmitPacket", 4), Mine("SubmitPacket"));
        Detour.Install(Find(n, "CreateDynamicSoundEffectInstance", 3), Mine("CreateDynamicSoundEffectInstance"));
        Detour.Install(Find(n, "SubmitBuffer", 3), Mine("SubmitBuffer"));
        Detour.Install(Find(n, "ClearPendingBuffers", 1), Mine("ClearPendingBuffers"));
        Detour.Install(Find(n, "GetPendingBufferCount", 2), Mine("GetPendingBufferCount"));
        Detour.Install(Find(n, "Play", 1), Mine("Play"));
        Detour.Install(Find(n, "Stop", 2), Mine("Stop"));
        Detour.Install(Find(n, "Pause", 1), Mine("Pause"));
        Detour.Install(Find(n, "SetVolume", 2), Mine("SetVolume"));
        Detour.Install(Find(n, "SetPitch", 2), Mine("SetPitch"));
        Detour.Install(Find(n, "SetPan", 2), Mine("SetPan"));
        // the pointer overload (the ref overload is small enough to be inlined into SoundEffectInstance.State)
        foreach (MethodInfo m in n.GetMethods(BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic))
            if (m.Name == "GetState" && m.GetParameters()[1].ParameterType.IsPointer) Detour.Install(m, Mine("GetState"));

        Type u = xact.GetType("Microsoft.Xna.Framework.Audio.UnsafeNativeMethods", true);
        Type e = u.GetNestedType("Engine", BindingFlags.Public | BindingFlags.NonPublic);
        Type w = u.GetNestedType("WaveBank", BindingFlags.Public | BindingFlags.NonPublic);
        Type c = u.GetNestedType("Cue", BindingFlags.Public | BindingFlags.NonPublic);
        Type s = u.GetNestedType("SoundBank", BindingFlags.Public | BindingFlags.NonPublic);
        Detour.Install(Find(e, "CreateHandle", 7), Mine("XEngine"));
        Detour.Install(Find(e, "Update", 1), Mine("XEngineUpdate"));
        Detour.Install(Find(e, "ReleaseHandle", 4), Mine("XEngineRelease"));
        Detour.Install(Find(e, "GetRendererCount", 1), Mine("XEngineRendererCount"));
        Detour.Install(Find(e, "GetRenderDetails", 4), Mine("XEngineRendererDetails"));
        Detour.Install(Find(e, "GetCategory", 3), Mine("XGetCategory"));
        Detour.Install(Find(e, "SetVolume", 3), Mine("XCatVolume"));
        Detour.Install(Find(e, "Pause", 3), Mine("XCatPause"));
        Detour.Install(Find(e, "Stop", 3), Mine("XCatStop"));
        Detour.Install(Find(e, "GetGlobalVariable", 4), Mine("XGetGlobal"));
        Detour.Install(Find(e, "SetGlobalVariable", 4), Mine("XSetGlobal"));
        Detour.Install(Find(w, "CreateHandle", 4), Mine("XWaveBank"));
        Detour.Install(Find(w, "CreateStreamingHandle", 4), Mine("XWaveBankStreaming"));
        Detour.Install(Find(w, "GetStatus", 1), Mine("XWaveBankStatus"));
        Detour.Install(Find(w, "ReleaseHandle", 2), Mine("XWaveBankRelease"));
        Detour.Install(Find(s, "CreateHandle", 4), Mine("XSoundBank"));
        Detour.Install(Find(s, "GetStatus", 1), Mine("XSoundBankStatus"));
        Detour.Install(Find(s, "ReleaseHandle", 2), Mine("XSoundBankRelease"));
        Detour.Install(Find(s, "GetCue", 4), Mine("XGetCue"));
        Detour.Install(Find(s, "PlayCue", 3), Mine("XPlayCue"));
        Detour.Install(Find(c, "GetStatus", 1), Mine("XCueStatus"));
        Detour.Install(Find(c, "Play", 1), Mine("XCuePlay"));
        Detour.Install(Find(c, "Pause", 2), Mine("XCuePause"));
        Detour.Install(Find(c, "Stop", 2), Mine("XCueStop"));
        Detour.Install(Find(c, "SetVariable", 4), Mine("XCueSetVariable"));
        Detour.Install(Find(c, "GetVariable", 4), Mine("XCueGetVariable"));
        Detour.Install(Find(c, "ReleaseHandle", 1), Mine("XCueRelease"));
    }
}
