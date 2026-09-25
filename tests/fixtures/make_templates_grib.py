#!/usr/bin/env python3
"""Build a GRIB2 file exercising grid definition templates beyond 3.0.

`sample.grib2` covers the plain lat/lon template. This one adds the three other
branches `GribRecordWriter.writeGridDefinition` dispatches to, so the port's
template parsing is checked against netCDF-Java rather than only against itself:

  * 3.1  rotated lat/lon      -> writeLonLatGrid + writeRotationAndStretch
  * 3.30 Lambert conformal    -> writeLambertConformalGrid
  * 3.90 space view           -> writeSpaceOrOrthographicGrid

Earth shape 1 (radius specified by the producer) is used on one message and
shape 3 (oblate, major/minor axes) on another, so both arms of the shape switch
in `writeGridShape` are exercised too.
"""
import struct, sys, pathlib

def u8(v):  return struct.pack(">B", v)
def u16(v): return struct.pack(">H", v)
def i16(v): return struct.pack(">h", v)
def u32(v): return struct.pack(">I", v)
def u64(v): return struct.pack(">Q", v)
def f32(v): return struct.pack(">f", v)

def deg(v):
    """A signed 1e-6-degree field: sign bit, then magnitude."""
    raw = int(round(abs(v) * 1e6))
    return u32(raw | 0x80000000) if v < 0 else u32(raw)

NI, NJ = 3, 2
NPTS = NI * NJ
E, D, NBITS = 0, 2, 16

def section1():
    b = u16(7) + u16(0) + u8(2) + u8(1) + u8(0)
    b += u16(2014) + u8(1) + u8(15) + u8(6) + u8(30) + u8(0)
    b += u8(0) + u8(1)
    return u32(5 + len(b)) + u8(1) + b

def earth(shape):
    """Octets 15-30: the shape block, shared by every template with a body."""
    if shape == 1:      # radius specified by the producer
        return u8(1) + u8(0) + u32(6371229) + u8(255) + u32(0xFFFFFFFF) + u8(255) + u32(0xFFFFFFFF)
    if shape == 3:      # oblate, major/minor axes in km
        return u8(3) + u8(255) + u32(0xFFFFFFFF) + u8(0) + u32(6378) + u8(0) + u32(6356)
    return u8(shape) + u8(0) + u32(0) + u8(0) + u32(0) + u8(0) + u32(0)

def gds(gdtn, shape, tail):
    head = u8(0) + u32(NPTS) + u8(0) + u8(0) + u16(gdtn)
    body = head + earth(shape) + tail
    return u32(5 + len(body)) + u8(3) + body

def rotated_latlon():
    """Template 3.1: the 3.0 body plus the southern-pole triple."""
    t  = u32(NI) + u32(NJ) + u32(0) + u32(0xFFFFFFFF)
    t += deg(60.0) + deg(10.0) + u8(0x30) + deg(50.0) + deg(12.0)
    t += deg(1.0) + deg(2.0) + u8(0)
    t += deg(-30.0) + deg(15.0) + f32(0.5)      # spLat, spLon, rotationAngle
    return gds(1, 1, t)

def lambert_conformal():
    """Template 3.30."""
    t  = u32(NI) + u32(NJ) + deg(25.0) + deg(-95.0) + u8(0x30)
    t += deg(40.0) + deg(-97.0)                  # LaD, LoV
    t += u32(5000) + u32(5000)                   # Dx, Dy (1e-6 deg here)
    t += u8(0) + u8(64)                          # projection flag, scanning mode
    t += deg(33.0) + deg(45.0)                   # Latin1, Latin2
    t += deg(-90.0) + deg(0.0)                   # southern pole lat/lon
    return gds(30, 3, t)

def space_view():
    """Template 3.90."""
    t  = u32(NI) + u32(NJ) + deg(0.0) + deg(140.7) + u8(0x30)
    t += u32(3000) + u32(3000)                   # Dx, Dy
    t += u32(1250000) + u32(1250000)             # Xp, Yp (1e-3 grid lengths)
    t += u8(0)                                   # scanning mode
    t += u32(0)                                  # orientation of the grid
    t += u32(6610700)                            # Nr (1e-6 earth radii)
    t += u32(0) + u32(0)                         # Xo, Yo
    return gds(90, 1, t)

def section4(param_number):
    t  = u8(0) + u8(param_number) + u8(0) + u8(0) + u8(96)
    t += u16(0) + u8(0) + u8(1) + u32(6)
    t += u8(100) + u8(0) + u32(50000)            # isobaric surface, 50000 Pa
    t += u8(255) + u8(255) + u32(0xFFFFFFFF)
    head = u16(0) + u16(0)
    return u32(5 + len(head) + len(t)) + u8(4) + head + t

def reference(values):
    r = min(values) * (10 ** D)
    return struct.unpack(">f", struct.pack(">f", r))[0]

def section5(values):
    t = f32(reference(values)) + i16(E) + i16(D) + u8(NBITS) + u8(0)
    head = u32(NPTS) + u16(0)
    return u32(5 + len(head) + len(t)) + u8(5) + head + t

def section6():
    return u32(6) + u8(6) + u8(255)

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

def message(discipline, param_number, grid, values):
    body = section1() + grid + section4(param_number) + section5(values) + section6() + section7(values)
    s0 = b"GRIB" + b"\x00\x00" + u8(discipline) + u8(2)
    total = len(s0) + 8 + len(body) + 4
    return s0 + u64(total) + body + b"7777"

A = [1.0, 2.5, -3.25, 0.0, 12.75, -0.5]
B = [-1.0, 0.25, 3.5, 7.0, -2.75, 1.25]
C = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0]

out = pathlib.Path(sys.argv[1])
out.write_bytes(
    message(0, 0, rotated_latlon(), A)
    + message(0, 1, lambert_conformal(), B)
    + message(0, 5, space_view(), C)
)
print("wrote", out, out.stat().st_size, "bytes")
