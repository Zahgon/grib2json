#!/usr/bin/env python3
"""Build a small classic NetCDF (CDF-1) file shaped like an OSCAR product.

Real OSCAR files carry a 481x1080 grid, which is ~2 MB per variable -- far too
large to commit as a fixture. This file keeps the same *structure* (a `time`
int, a `depth` double, and `u`/`v` floats over time/depth/lat/lon) at a 3x4
grid, which exercises the header parser, the scalar reads and the strided
section read without the bulk. The OSCAR header path never reads u/v data, so
it is fully exercised at this size; `read_section` is tested against the small
grid directly.
"""
import struct, sys, pathlib

def u32(v): return struct.pack(">I", v)

def name(s):
    b = s.encode()
    return u32(len(b)) + b + b"\x00" * ((4 - len(b) % 4) % 4)

NC_DIMENSION, NC_VARIABLE, NC_ATTRIBUTE = 0x0A, 0x0B, 0x0C
NC_INT, NC_FLOAT, NC_DOUBLE = 4, 5, 6

DIMS = [("time", 1), ("depth", 1), ("latitude", 3), ("longitude", 4)]
NLAT, NLON = 3, 4

# (name, dimids, type, values)
U = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, float("nan")]
V = [-1.0, -2.0, -3.0, -4.0, -5.0, -6.0, -7.0, -8.0, -9.0, -10.0, -11.0, -12.0]
VARS = [
    ("time",  [0],          NC_INT,    [7700]),
    ("depth", [1],          NC_DOUBLE, [15.0]),
    ("u",     [0, 1, 2, 3], NC_FLOAT,  U),
    ("v",     [0, 1, 2, 3], NC_FLOAT,  V),
]

TYPE_SIZE = {NC_INT: 4, NC_FLOAT: 4, NC_DOUBLE: 8}

def pack(ty, values):
    fmt = {NC_INT: ">i", NC_FLOAT: ">f", NC_DOUBLE: ">d"}[ty]
    return b"".join(struct.pack(fmt, v) for v in values)

header = b"CDF\x01" + u32(0)                       # magic, version, numrecs
header += u32(NC_DIMENSION) + u32(len(DIMS))
for n, size in DIMS:
    header += name(n) + u32(size)
header += u32(0) + u32(0)                          # global attributes: ABSENT
header += u32(NC_VARIABLE) + u32(len(VARS))

# Two passes: the header's size is only known once every entry is laid out, and
# each entry has to carry the absolute file offset of its data.
def build(begins):
    b = b""
    for (n, dimids, ty, values), begin in zip(VARS, begins):
        b += name(n) + u32(len(dimids))
        for d in dimids:
            b += u32(d)
        b += u32(0) + u32(0)                       # variable attributes: ABSENT
        vsize = len(values) * TYPE_SIZE[ty]
        vsize += (4 - vsize % 4) % 4
        b += u32(ty) + u32(vsize) + u32(begin)
    return b

begins = [0] * len(VARS)
for _ in range(4):                                 # converges on the first pass
    size = len(header) + len(build(begins))
    off = size
    new = []
    for _n, _d, ty, values in VARS:
        new.append(off)
        vsize = len(values) * TYPE_SIZE[ty]
        off += vsize + (4 - vsize % 4) % 4
    if new == begins:
        break
    begins = new

out = header + build(begins)
for _n, _d, ty, values in VARS:
    blob = pack(ty, values)
    out += blob + b"\x00" * ((4 - len(blob) % 4) % 4)

path = pathlib.Path(sys.argv[1])
path.write_bytes(out)
print("wrote", path, len(out), "bytes")
