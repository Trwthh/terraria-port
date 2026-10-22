#!/usr/bin/env python3
"""Sets IMAGE_FILE_LARGE_ADDRESS_AWARE in the PE header of an executable (argument 1), in place.

RefDumpWorldgenX.exe hosts the game's code in its own 32-bit process. TerrariaServer.exe has this flag (4 GB of
address space on 64-bit Windows); csc has no switch for it, and a large world (20 million Tile objects) does not fit
into 2 GB together with the harness's buffers. Only the harness's own executable is patched, never a game file.
"""
import struct
import sys

path = sys.argv[1]
if "refserver" in path.lower() or "/games/" in path.lower().replace("\\", "/"):
    sys.exit("set_laa.py: refusing to touch a game file: " + path)
with open(path, "r+b") as f:
    data = f.read(4096)
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        sys.exit("set_laa.py: not a PE file: " + path)
    off = pe + 4 + 18
    flags = struct.unpack_from("<H", data, off)[0]
    f.seek(off)
    f.write(struct.pack("<H", flags | 0x20))
print("set_laa.py: %s characteristics %#06x -> %#06x" % (path, flags, flags | 0x20))
