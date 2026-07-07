// Regression test: run the full sample app with mock button inputs
// and verify PBM output matches golden files exactly.
//
// Golden files live in tests/golden/. Populate them manually:
//   cargo run -p xtx4_sample --target x86_64-unknown-linux-gnu --features mock
//   cp /tmp/xtx4_frames/frame_*.pbm xtx4_platform/tests/golden/
// Then run: cargo test -p xtx4_platform --test regression --target x86_64-unknown-linux-gnu --features mock

#![cfg(all(target_arch = "x86_64", feature = "mock"))]

use std::process::Command;

const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden");

fn run_sample() {
    let _ = std::fs::remove_dir_all("/tmp/xtx4_frames");
    std::fs::create_dir_all("/tmp/xtx4_frames").unwrap();

    let status = Command::new("cargo")
        .args(["run", "-p", "xtx4_sample", "--target", "x86_64-unknown-linux-gnu", "--features", "mock"])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn regression() {
    run_sample();

    // Check counts match first.
    let golden_count = std::fs::read_dir(GOLDEN_DIR).unwrap().count();
    let generated_count = std::fs::read_dir("/tmp/xtx4_frames").unwrap().count();
    assert_eq!(generated_count, golden_count, "Frame count mismatch");

    // Then compare each file byte-for-byte.
    for entry in std::fs::read_dir(GOLDEN_DIR).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        let expected = std::fs::read(entry.path()).unwrap();
        let actual = std::fs::read(format!("/tmp/xtx4_frames/{}", name))
            .unwrap_or_else(|_| panic!("Missing generated file: {}", name));
        assert_eq!(expected, actual, "Frame {} differs from golden", name);
    }
}