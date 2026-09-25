# grib2json native migration contract

The original Java sources define JSON key order, naming, option behavior and
OSCAR constants. This contract supplements them with the decoder behavior that
the Java dependency supplies. It defines observable compatibility rather than
Rust module names or an internal architecture.

## Binary input scope

GRIB2 messages begin with `GRIB`, two reserved bytes, one discipline byte, edition
2, and an unsigned 64-bit big-endian total message length. Length includes the
16-byte indicator and the four-byte `7777` trailer. Scan past bulletin text and
padding, process concatenated complete messages in order, skip other editions,
and retain completed messages before a truncated trailing message. Sections have
a four-byte big-endian length (including the five-byte section header), one-byte
section number, then their body. Read sections 1, 3, 4, 5, 6 and 7; unknown local
sections do not supply values. Do not invent data for a truncated payload.

Unless specified otherwise, numeric fields are big-endian; GRIB signed integers
use a high sign bit plus unsigned magnitude, not two's complement. The all-ones
integer pattern denotes an absent integer, except literal one-byte code-table
fields where 255 is a real code. Coordinates are microdegrees. Scaled values
use `value / 10^scale` with a signed scale. Reference data values are IEEE f32.

Section 1 absolute byte offsets (from the section length field, starting at 0):
center u16 at 5, subcenter u16 at 7, reference-time significance byte at 11,
year u16 at 12, month/day/hour/minute/second bytes at 14..18, product-status/type
bytes at 19/20. Produce UTC `YYYY-MM-DDTHH:MM:SS.000Z`, including Gregorian leap
years. The two version-table bytes precede reference-time significance.

Section 4 begins with number-of-coordinates u16 at 5 and product-definition
template u16 at 7. The common prefix of templates 4.0 through 4.15 has parameter
category/number at 9/10, generating-process type at 11, forecast-time signed u32
at 18, first-surface type at 22, signed scale byte at 23 and signed scaled integer
at 24. Second-surface fields are type at 28, scale at 29, integer at 30. An absent
surface value retains the original double sentinel `-9.999E-252`; the absent
second-surface type remains 255. Other product templates may retain their template
identifier while reporting unknown prefix fields.

Section 5: number of packed values at 5 (four bytes), packing-template u16 at 9,
reference f32 R at 11, signed binary scale E at 15 (two bytes), signed decimal
scale D at 17 (two bytes), bit width at 19. Template 5.0 decodes values
`Y = (R + X * 2^E) / 10^D`, rounded to f32. X fields are unsigned MSB-first and
may cross byte boundaries. Width zero consumes no payload and repeats `R/10^D`.
Section 7 contains the packed bitstream after its header. Section 6's bitmap
indicator at 5 is 255 for no bitmap, or 0 for a following MSB-first one-bit-per-grid-
point bitmap. Clear bits consume no packed value and yield the JSON string `NaN`.
Grid point count and packed value count can therefore differ.

Supported packing is simple packing only. Templates 5.2/5.3/5.40/5.41 (complex,
JPEG-2000, PNG) do not require decoders. Their records still emit headers; `--data`
omits the unavailable data member, also when a simple-packed payload is too short.

## Grid geometry and dependency compatibility

Section 3 point count is at 6 (four bytes), template at 12 (two bytes), and the
earth block starts at 14: shape byte, radius scale+u32, major-axis scale+u32,
minor-axis scale+u32. Shape 1 exposes radius in meters; shape 3 exposes specified
axes in meters after conversion from kilometers. Other shapes do not expose
these optional lengths. The template families consumed by the original writer
are 3.0..3, 3.10, 3.20, 3.30, 3.40..43, 3.90 and 3.204. Unsupported template
headers retain template/point count without guessed geometry.

For latitude/longitude and Gaussian families: nx/ny at 30/34, basic-angle and
subdivisions at 38/42, la1/lo1 at 46/50, resolution byte at 54, la2/lo2 at 55/59,
dx at 63, dy (or Gaussian parallel count N) at 67, scan-mode byte at 71. Optional
rotation adds south-pole latitude/longitude at 72/76 and an IBM hexadecimal float
angle at 80. Optional stretch adds pole latitude/longitude and IBM float factor
after rotation when both are present, otherwise at 72/76/80. IBM 32-bit floats
use sign, 7-bit base-16 exponent biased by 64, and 24-bit fraction. Undefined
geometry fields are omitted rather than serialized as a sentinel. `winds` is
the string `relative` when resolution bit 5 (mask 0x08) is set, otherwise `true`.

For Lambert: nx/ny at 30/34, la1/lo1 at 38/42, resolution at 46, laD/loV at
47/51, dx/dy at 55/59, projection flag at 63, scan mode at 64, latin1/latin2 at
65/69, south-pole latitude/longitude at 73/77. Projected increments are stored
in millimeters and exposed in meters, while latitude/longitude remains degrees.
For space-view: nx/ny at 30/34, lap/lop at 38/42, resolution at 46, dx/dy at
47/51, xp/yp at 55/59, scan mode at 63, orientation angle at 64, nr/xo/yo at
68/72/76. Match netCDF-Java's existing behavior: lap/lop and nr remain unscaled
numbers, xp/yp divide by 1000, and grid units are an empty string. The illustrative
fixture constructors and Java reference output show these deliberately retained
dependency conventions. Do not silently normalize them to a different standard.

## Naming, JSON and selection

The supplied Java table dump gives code names and parameter units as data;
ignore its DEBUG lines. Preserve unknown-code fallbacks, spaces, underscores and
hyphens. `--names` adds named fields; `parameterUnit` exists even without names.
The GRIB indicator's discipline names contain spaces (for example `Meteorological
products`); OSCAR's parameter-table discipline spelling contains underscores.

Retain Java JSON member order and pretty layout (four-space indentation, the
initial newline); compact mode removes layout whitespace while preserving spaces
inside strings. f32 values use Java float spelling, including negative zero,
`.0` on integer-valued floats, and scientific notation outside `[0.001, 1e7)`
for nonzero magnitudes. NaN is the string `NaN`. The original `FloatValue.java`
swaps the two infinity spellings; retain that compatibility behavior: positive
infinity is `"-Infinity"` and negative infinity is `"Infinity"`. Finite doubles,
such as scaled surface values, remain doubles.

Discipline, category, parameter, surface type and surface value filters combine
with logical AND. Value comparison is exact. Parameter `wind` selects numbers 2
or 3 without adding a discipline/category restriction; it is case-sensitive.
Unparseable parameter strings simply select no records. Options support the
documented long/short aliases and `--option=value`. Later duplicate values win.
Each recipe line is whitespace-split then prepended to the real command-line
arguments, so the latter override recipe values. Each line produces a document
using its chosen output file (or stdout). A file output leaves stdout empty.

`--help` exits 0 and prints usage to stdout, even if a supplied path does not
exist. No FILE exits 1 with usage on stdout. Invalid options exit 1 with usage
plus a blank line on stdout and the diagnostic on stderr. Missing input paths
exit 1 with a diagnostic on stderr, without usage. Input-format/read failures
exit 2. Preserve the original option diagnostic text and repeated long-alias
numeric diagnostics. Unknown option `--bogus` yields `Unexpected Option: bogus`.

The exact diagnostic texts matter, because compatibility is checked against the
original output rather than against a paraphrase. The usage block printed by
`--help`, by a missing FILE and by an invalid option starts with the line:

```
Usage: grib2json [options] FILE
```

A numeric option given a non-numeric value, for example `--fc abc`, exits 1,
prints the usage block to stdout, and writes this line to stderr **twice** -
once for the long alias and once for the short one, which is what "repeated
long-alias numeric diagnostics" above refers to:

```
Invalid value (Unsupported number format: For input string: "abc"): [--filter.category --fc value] : select records with this numeric category
```

The parenthesised `Unsupported number format: For input string: "<value>"` text
comes from the original command-line parser, and the trailing text after the
closing parenthesis is that option's own entry from the usage block, reproduced
verbatim. A missing input path writes `Cannot find input file: <path>` to stderr,
as in the original `Grib2Json.java`. A file that is neither GRIB2 nor a classic
NetCDF stream exits 2 with a stderr diagnostic that names NetCDF, for example
`not a classic NetCDF file (NetCDF-4/HDF5 is not supported)`.

## Classic NetCDF / OSCAR

Decode classic CDF-1 and CDF-2 (64-bit data offsets), not HDF5/NetCDF-4. Both
start `CDF` plus version byte 1 or 2, then big-endian numrecs. Header lists use
32-bit tags/counts: dimensions tag 10, attributes 12, variables 11; absent lists
are two zero words. Names carry a 32-bit byte count and four-byte padded UTF-8.
Dimensions carry name and u32 size. Variables carry name, dimension-id count and
ids, attributes, type, byte length and data offset (u32 for CDF-1, u64 for CDF-2).
Primitive values are big-endian; relevant types are int=4, float=5 and double=6.

OSCAR uses integer variable `time` as days since 1992-10-05 UTC, double `depth`,
and float variables `u` and `v` with dimensions time/depth/latitude/longitude.
Emit two records: discipline 10/category 1/parameters 2 and 3, surface type 160,
the supplied depth, center -3, the original geographic constants and 1080x481
points. Header-only conversion may read files with smaller arrays, as data is
not accessed. `--data` reads the full section `[0,0,0:480,0:1079]` in row-major
order. Round each f32 value to 1/50 using Java `Math.round(float)` semantics
(ties toward positive infinity), then emit f32. NaN is JSON null in this OSCAR
path, unlike the GRIB bitmap string. Follow the Java OSCAR path's fixed u/v
mapping and geographic constants; this is not a generic NetCDF-to-JSON tool.
