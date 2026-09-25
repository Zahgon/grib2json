grib2json
=========

A command line utility that decodes [GRIB2](http://en.wikipedia.org/wiki/GRIB) files as JSON.

Rust port of [cambecc/grib2json](https://github.com/cambecc/grib2json) at commit
`7fb455d745854e336a4462808aca8d791232ef84`. The original uses the netCDF-Java GRIB decoder,
part of the [THREDDS](https://github.com/Unidata/thredds) project by University Corporation
for Atmospheric Research/Unidata; this port decodes GRIB2 itself. See [MIGRATION.md](MIGRATION.md)
for what that involved and how the two are shown to agree.

Installation
------------

```
cargo build --release
```

The binary is `target/release/grib2json`. There are no dependencies to fetch — the crate has none.

Usage
-----

```
> grib2json --help
Usage: grib2json [options] FILE
	[--compact -c] : enable compact Json formatting
	[--data -d] : print GRIB record data
	[--filter.category --fc value] : select records with this numeric category
	[--filter.discipline --fd value] : select records with this discipline
	[--filter.parameter --fp value] : select records with this numeric parameter, or the string "wind" for both u,v components
	[--filter.surface --fs value] : select records with this numeric surface type
	[--filter.value --fv value] : select records with this numeric surface value
	[--help -h] : display this help
	[--names -n] : print names of numeric codes
	[--output -o value] : write output to the specified file (default is stdout)
	[--recipe -r value] : a file containing a batch of filter options: fd, fc, fp, fs, fv, and o
	[--verbose -v] : enable logging to stdout
```

For example, the following command outputs to stdout the records for parameter 2 (U-component_of_wind), with
surface type 103 (Specified height level above ground), and surface value 10.0 meters from the GRIB2 file
_gfs.t18z.pgrbf00.2p5deg.grib2_. Notice the optional inclusion of human-readable _xyzName_ keys and the data array:

```
> grib2json --names --data --fp 2 --fs 103 --fv 10.0 gfs.t18z.pgrbf00.2p5deg.grib2

[
    {
        "header":{
            "discipline":0,
            "disciplineName":"Meteorological products",
            "parameterNumber":2,
            "parameterNumberName":"U-component_of_wind",
            "parameterUnit":"m.s-1",
            "surface1Type":103,
            "surface1TypeName":"Specified height level above ground",
            "surface1Value":10.0,
            ...
        },
        "data":[
            -2.12,
            -2.27,
            -2.41,
            ...
        ]
    }
]
```

Tests
-----

```
cargo test
```

57 tests. Six of them decode a committed GRIB2 fixture and assert the output matches, byte
for byte, what the Java build printed for the same bytes; five more check all 1,373 code-table
entries against netCDF-Java's own answers. See [MIGRATION.md](MIGRATION.md).

Supported input
---------------

GRIB2 with simple packing (data representation template 5.0), across grid definition templates
3.0–3.3, 3.10, 3.20, 3.30, 3.31, 3.40–3.43, 3.90 and 3.204; and classic NetCDF (CDF-1/CDF-2)
for OSCAR ocean-current products. Complex, JPEG-2000 and PNG packing are not yet implemented —
a record packed that way is emitted with its header and no `data` array, which is what the
original does when its decoder cannot unpack a record. MIGRATION.md has the full list.

License
-------

MIT, © 2013 Cameron Beccario. See [LICENSE.md](LICENSE.md).
