import struct, sys
p = sys.argv[1]; b = bytearray(open(p, 'rb').read())
pe = struct.unpack_from('<I', b, 0x3c)[0]
assert b[pe:pe+4] == b'PE\0\0'
nsec = struct.unpack_from('<H', b, pe + 6)[0]; optsz = struct.unpack_from('<H', b, pe + 20)[0]
opt = pe + 24; magic = struct.unpack_from('<H', b, opt)[0]
dd = opt + (96 if magic == 0x10b else 112)
cli_rva = struct.unpack_from('<I', b, dd + 14 * 8)[0]
sec = opt + optsz
for i in range(nsec):
    s = sec + 40 * i
    vsize, va, rsize, raw = struct.unpack_from('<IIII', b, s + 8)
    if va <= cli_rva < va + max(vsize, rsize):
        off = cli_rva - va + raw; break
flags = struct.unpack_from('<I', b, off + 16)[0]
struct.pack_into('<I', b, off + 16, flags | 8)
open(p, 'wb').write(b); print(p, 'flags', hex(flags), '->', hex(flags | 8))
