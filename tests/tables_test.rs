//! Every code-table entry, checked against netCDF-Java's own answers.
//!
//! `tests/fixtures/netcdf_java_tables.txt` is the output of
//! `docker/grib2json-DumpTables.java`, run on the classpath of the Java build at
//! commit `7fb455d7`. It calls each accessor across its code range and prints
//! what came back. This test walks that file and asserts the Rust table agrees,
//! entry for entry.
//!
//! That is worth doing rather than spot-checking a handful, because a wrong
//! string in a lookup table has no symptom other than a wrong `--names` header
//! for one particular code -- the sort of thing that survives every other test
//! and shows up in production.

mod common;

use common::fixture;
use grib2json::grib::tables;
use std::collections::HashMap;

/// Split the dump into its `### section` blocks.
fn sections() -> HashMap<String, Vec<Vec<String>>> {
    let text = std::fs::read_to_string(fixture("netcdf_java_tables.txt"))
        .expect("the netCDF-Java table dump is committed alongside the fixtures");
    let mut out: HashMap<String, Vec<Vec<String>>> = HashMap::new();
    let mut current = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("### ") {
            current = rest.split(' ').next().unwrap_or(rest).to_string();
            out.entry(current.clone()).or_default();
        } else if !line.trim().is_empty() && !current.is_empty() {
            let cols: Vec<String> = line.split('\t').map(str::to_string).collect();
            if cols[0].parse::<i32>().is_ok() {
                out.get_mut(&current).unwrap().push(cols);
            }
        }
    }
    out
}

/// Assert a single-argument table over every code the dump carries.
fn check<F: Fn(i32) -> String>(rows: &[Vec<String>], label: &str, f: F) -> usize {
    assert!(!rows.is_empty(), "{label}: the dump carries no rows");
    for row in rows {
        let code: i32 = row[0].parse().unwrap();
        assert_eq!(f(code), row[1], "{label}({code})");
    }
    rows.len()
}

#[test]
fn every_single_argument_table_matches_netcdf_java() {
    let s = sections();
    let mut checked = 0;
    checked += check(&s["codeTable3_1"], "codeTable3_1", tables::code_table3_1);
    checked += check(&s["codeTable3_2"], "codeTable3_2", |c| tables::code_table3_2(c).to_string());
    checked += check(&s["codeTable4_0"], "codeTable4_0", |c| tables::code_table4_0(c).to_string());
    checked += check(&s["codeTable4_3"], "codeTable4_3", |c| tables::code_table4_3(c).to_string());
    checked += check(&s["codeTable4_5"], "codeTable4_5", tables::code_table4_5);
    checked += check(&s["center_id"], "getCenter_idName", |c| tables::center_id_name(c).to_string());
    checked += check(&s["discipline"], "getDisciplineName", tables::parameter_table_discipline_name);
    assert!(checked > 800, "expected the full dump, only checked {checked} entries");
}

#[test]
fn every_category_matches_netcdf_java() {
    let s = sections();
    let rows = &s["categories"];
    assert!(rows.len() > 200, "only {} categories in the dump", rows.len());
    for row in rows {
        let (d, c): (i32, i32) = (row[0].parse().unwrap(), row[1].parse().unwrap());
        assert_eq!(tables::category_name(d, c), row[2], "getCategoryName({d}, {c})");
    }
}

#[test]
fn every_parameter_matches_netcdf_java() {
    let s = sections();
    let rows = &s["parameters"];
    assert!(rows.len() > 200, "only {} parameters in the dump", rows.len());
    for row in rows {
        let (d, c, n): (i32, i32, i32) =
            (row[0].parse().unwrap(), row[1].parse().unwrap(), row[2].parse().unwrap());
        assert_eq!(tables::parameter_name(d, c, n), row[3], "getParameterName({d}, {c}, {n})");
        let unit = row.get(4).cloned().unwrap_or_default();
        assert_eq!(tables::parameter_unit(d, c, n), unit, "getParameterUnit({d}, {c}, {n})");
    }
}

/// The fallbacks, over codes the dump shows netCDF-Java has no entry for.
#[test]
fn fallbacks_match_netcdf_java() {
    assert_eq!(tables::code_table3_1(7), "Unknown projection7");
    assert_eq!(tables::code_table3_1(255), "Unknown projection255");
    assert_eq!(tables::code_table3_2(50), "Unknown Earth Shape");
    assert_eq!(tables::code_table4_0(200), "Unknown");
    assert_eq!(tables::code_table4_3(200), "Unknown");
    assert_eq!(tables::code_table4_5(99), "Unknown=99");
    assert_eq!(tables::center_id_name(999), "Unknown");
    assert_eq!(tables::parameter_table_discipline_name(99), "UnknownDiscipline_99");
    assert_eq!(tables::category_name(0, 99), "UnknownCategory_99");
    assert_eq!(tables::parameter_name(0, 2, 99), "UnknownParameter_D0_C2_99");
    assert_eq!(tables::parameter_unit(0, 2, 99), "Unknown");
}

/// The indicator section's discipline names, which are a *different* table from
/// `ParameterTable`'s and are spelled with spaces rather than underscores.
#[test]
fn indicator_discipline_names_are_the_spaced_spelling() {
    assert_eq!(tables::discipline_name(0), "Meteorological products");
    assert_eq!(tables::discipline_name(1), "Hydrological products");
    assert_eq!(tables::discipline_name(2), "Land surface products");
    assert_eq!(tables::discipline_name(3), "Space products");
    assert_eq!(tables::discipline_name(10), "Oceanographic products");
    assert_eq!(tables::discipline_name(77), "Unknown");
    // And they really do differ from the ParameterTable spelling.
    assert_ne!(tables::discipline_name(0), tables::parameter_table_discipline_name(0));
}
