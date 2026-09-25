#!/bin/sh
# Reproduce the equivalence check: decode the same GRIB2 bytes with both
# implementations and diff the two documents.
#
#   sh equivalence/run.sh
#
# Run from the repository root (code_migrations_task), with both test images
# already built:
#
#   docker build --target test-report -t grib2json-test:local \
#                -f docker/grib2json.Dockerfile \
#                scraped_repos/Java/cambecc_grib2json
#   docker build -f docker/grib2json-rust-test.Dockerfile \
#                -t grib2json-rust-test migrated_repo_rust/cambecc_grib2json
#
# The Java half needs no probe script: grib2json is a command line program, so
# the observation is simply its stdout. That is the whole reason the boundary is
# observable here at all -- there is no wire to intercept, the document *is* the
# wire.
#
# Exits 0 when the two outputs are identical for every fixture.

set -e

ROOT=$(pwd)
MIG=$ROOT/migrated_repo_rust/cambecc_grib2json
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

FIXTURES="sample.grib2 templates.grib2"
ARGS="--names --data"

status=0
for f in $FIXTURES; do
    echo "=== $f ==="

    # Java: run the compiled classes against the fixture, offline.
    docker run --rm --network none -v "$MIG/tests/fixtures":/f --entrypoint bash \
        grib2json-test:local -c \
        "java -cp \"/app/target/classes:\$(find \$M2 -name '*.jar' | tr '\n' ':')\" \
         net.nullschool.grib2json.Launcher $ARGS /f/$f" > "$WORK/java_$f.json"

    # Rust: the same arguments, the same bytes.
    docker run --rm --network none -v "$MIG/tests/fixtures":/f \
        --entrypoint cargo grib2json-rust-test \
        run --locked --offline --quiet --release -- $ARGS "/f/$f" > "$WORK/rust_$f.json"

    if diff -u "$WORK/java_$f.json" "$WORK/rust_$f.json"; then
        echo "identical"
    else
        echo "DIVERGED on $f"
        status=1
    fi
done

# The committed captures must still be what Java prints -- otherwise the suite is
# asserting against a stale golden and would not notice a regression.
echo "=== committed captures still current ==="
diff -u "$MIG/tests/fixtures/java_names_data.json" "$WORK/java_sample.grib2.json" || status=1
diff -u "$MIG/tests/fixtures/java_templates.json"  "$WORK/java_templates.grib2.json" || status=1

exit $status
