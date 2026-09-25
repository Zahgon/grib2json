//! Port of `ucar.grib.grib2.Grib2Input` -- finding and splitting messages.

use super::numbers::{code, uint};
use super::record::{
    message_length, parse_drs, parse_gds, parse_identification, parse_pds, Gds, Grib2Record,
    IndicatorSection, Pds,
};
use super::record::{DataRepresentation, IdentificationSection};

/// Scans a byte buffer for GRIB2 messages.
pub struct Grib2Input {
    buf: Vec<u8>,
    records: Vec<Grib2Record>,
}

impl Grib2Input {
    pub fn new(buf: Vec<u8>) -> Self {
        Grib2Input { buf, records: Vec::new() }
    }

    /// `Grib2Input.scan(boolean getProducts, boolean oneRecord)`.
    ///
    /// Returns false when the buffer holds no GRIB message at all, which is the
    /// signal `Grib2Json.write()` uses to fall through to the NetCDF path.
    pub fn scan(&mut self) -> bool {
        let buf = std::mem::take(&mut self.buf);
        let mut pos = 0usize;
        let mut found = false;
        while pos + 16 <= buf.len() {
            if &buf[pos..pos + 4] != b"GRIB" {
                pos += 1;
                continue;
            }
            let total = message_length(&buf, pos);
            if total <= 16 || pos + total as usize > buf.len() {
                break;
            }
            let msg = &buf[pos..pos + total as usize];
            if let Some(record) = Self::parse_message(msg) {
                self.records.push(record);
                found = true;
            }
            pos += total as usize;
        }
        self.buf = buf;
        found
    }

    /// Split one message into its sections and build the record.
    fn parse_message(msg: &[u8]) -> Option<Grib2Record> {
        let is = IndicatorSection {
            discipline: code(msg, 6),
            grib_edition: code(msg, 7),
            grib_length: message_length(msg, 0),
        };
        if is.grib_edition != 2 {
            return None;
        }

        let mut id: Option<IdentificationSection> = None;
        let mut gds: Option<Gds> = None;
        let mut pds: Option<Pds> = None;
        let mut drs: Option<DataRepresentation> = None;
        let mut bitmap: Option<Vec<u8>> = None;
        let mut data_bytes: Vec<u8> = Vec::new();

        // Sections run from octet 17 to the "7777" terminator.
        let mut off = 16usize;
        while off + 5 <= msg.len() {
            if &msg[off..(off + 4).min(msg.len())] == b"7777" {
                break;
            }
            let len = uint(msg, off, 4);
            if len <= 0 || off + len as usize > msg.len() {
                break;
            }
            let section = &msg[off..off + len as usize];
            match code(section, 4) {
                1 => id = Some(parse_identification(section)),
                3 => gds = Some(parse_gds(section)),
                4 => pds = Some(parse_pds(section)),
                5 => drs = Some(parse_drs(section)),
                6 => {
                    // Octet 6 is the bitmap indicator; 255 means "no bitmap".
                    if code(section, 5) != 255 && section.len() > 6 {
                        bitmap = Some(section[6..].to_vec());
                    }
                }
                7 => data_bytes = section[5..].to_vec(),
                _ => {}
            }
            off += len as usize;
        }

        Some(Grib2Record {
            is,
            id: id?,
            pds: pds?,
            gds: gds.unwrap_or_default(),
            drs,
            bitmap,
            data_bytes,
        })
    }

    /// `Grib2Input.getRecords()`.
    pub fn records(&self) -> &[Grib2Record] {
        &self.records
    }
}
