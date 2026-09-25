#!/usr/bin/env python3
"""Build a minimal, valid GRIB2 message so both implementations can decode the
same bytes. Two records: parameter 2 (U wind) and parameter 3 (V wind), both on
a 4x3 lat/lon grid (template 3.0) with simple packing (template 5.0)."""
import struct, sys, pathlib

def u8(v):  return struct.pack(">B", v)
def u16(v): return struct.pack(">H", v)
def i16(v): return struct.pack(">h", v)
def u32(v): return struct.pack(">I", v)
def u64(v): return struct.pack(">Q", v)
def f32(v): return struct.pack(">f", v)

NI, NJ = 4, 3
NPTS = NI * NJ

def section1():
    body = (u8(1)                      # centre: 1? use 7 = NCEP
            )
    b = b""
    b += u16(7)      # centre  (NCEP)
    b += u16(4)      # subcentre
    b += u8(2)       # master tables version
    b += u8(1)       # local tables version
    b += u8(1)       # significance of reference time: 1 = start of forecast
    b += u16(2013)   # year
    b += u8(10)      # month
    b += u8(24)      # day
    b += u8(18)      # hour
    b += u8(0)       # minute
    b += u8(0)       # second
    b += u8(0)       # production status: operational
    b += u8(1)       # type of data: forecast products
    return u32(5 + len(b)) + u8(1) + b

def section3():
    t = b""
    t += u8(6)          # shape of earth: 6 = spherical, radius 6371229 m
    t += u8(0) + u32(0) # scale factor / scaled value of radius
    t += u8(0) + u32(0) # major axis
    t += u8(0) + u32(0) # minor axis
    t += u32(NI)        # Ni
    t += u32(NJ)        # Nj
    t += u32(0)         # basic angle
    t += u32(0xFFFFFFFF)  # subdivisions: missing
    t += u32(int(80.0 * 1e6))    # La1 = 80.000000
    t += u32(int(0.0 * 1e6))     # Lo1 = 0
    t += u8(0x30)       # resolution and component flags: bits 3,4 set, bit5 clear -> "true"
    t += u32(int(70.0 * 1e6))    # La2
    t += u32(int(3.0 * 1e6))     # Lo2
    t += u32(int(1.0 * 1e6))     # Di
    t += u32(int(5.0 * 1e6))     # Dj
    t += u8(0)          # scanning mode
    head = u8(0) + u32(NPTS) + u8(0) + u8(0) + u16(0)   # source, npts, optlen, interp, gdtn=0
    return u32(5 + len(head) + len(t)) + u8(3) + head + t

def section4(param_number):
    t = b""
    t += u8(2)          # parameter category: 2 = momentum
    t += u8(param_number)
    t += u8(2)          # type of generating process: 2 = forecast
    t += u8(0)          # background process
    t += u8(96)         # generating process identifier
    t += u16(0)         # hours after cutoff
    t += u8(0)          # minutes after cutoff
    t += u8(1)          # indicator of unit of time range: hour
    t += u32(0)         # forecast time
    t += u8(103)        # type of first fixed surface: specified height above ground
    t += u8(0)          # scale factor
    t += u32(10)        # scaled value -> 10.0 m
    t += u8(255)        # type of second fixed surface: missing
    t += u8(255)        # scale factor: missing
    t += u32(0xFFFFFFFF)  # scaled value: missing
    head = u16(0) + u16(0)   # number of coordinates, pdtn = 0
    return u32(5 + len(head) + len(t)) + u8(4) + head + t

# Simple packing: value = (R + X * 2^E) / 10^D, so X = (Y*10^D - R) / 2^E.
# R has to sit at or below the smallest scaled value or X goes negative, which
# simple packing cannot express (X is an unsigned bit field).
E, D, NBITS = 0, 2, 16

def reference(values):
    r = min(values) * (10 ** D)
    # Store what an f32 can actually hold, so the decoder recovers the same R.
    return struct.unpack(">f", struct.pack(">f", r))[0]

def section5(values):
    t = f32(reference(values)) + i16(E) + i16(D) + u8(NBITS) + u8(0)
    head = u32(NPTS) + u16(0)
    return u32(5 + len(head) + len(t)) + u8(5) + head + t

def section6():
    return u32(6) + u8(6) + u8(255)   # 255 = no bitmap

def section7(values):
    R = reference(values)
    bits = ""
    for v in values:
        x = round((v * (10 ** D) - R) / (2 ** E))
        assert 0 <= x < 2 ** NBITS, (v, x)
        bits += format(x, "0%db" % NBITS)
    while len(bits) % 8:
        bits += "0"
    data = bytes(int(bits[i:i + 8], 2) for i in range(0, len(bits), 8))
    return u32(5 + len(data)) + u8(7) + data

def message(discipline, param_number, values):
    body = section1() + section3() + section4(param_number) + section5(values) + section6() + section7(values)
    s0 = b"GRIB" + b"\x00\x00" + u8(discipline) + u8(2)
    total = len(s0) + 8 + len(body) + 4
    return s0 + u64(total) + body + b"7777"

U = [-2.12, -2.27, -2.41, 0.0, 1.5, 3.25, 10.0, 0.01, 99.99, -50.5, 7.77, 0.5]
V = [1.0, 2.0, 3.0, 4.0, 5.5, 6.25, 0.0, -1.5, -20.0, 33.33, 0.02, 12.5]

out = pathlib.Path(sys.argv[1])
out.write_bytes(message(0, 2, U) + message(0, 3, V))
print("wrote", out, out.stat().st_size, "bytes")
