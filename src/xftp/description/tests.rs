//! Hostile description text: nesting, line counts and random shapes are refused without a crash.
#![cfg(not(target_arch = "wasm32"))]
use super::FileDescription;

/// Runs `f` on a thread with a small stack, as wasm and spawned threads have.
fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn a_line_of_ten_thousand_dashes_is_refused_without_overflowing_the_stack() {
    on_small_stack(|| {
        let text = format!("{}x", "- ".repeat(10_000));
        let e = FileDescription::parse(&text).unwrap_err();
        assert!(e.contains("nested too deeply"), "{e}");
    });
}

#[test]
fn deeply_indented_maps_are_refused_without_overflowing_the_stack() {
    on_small_stack(|| {
        let text: String = (0..10_000)
            .map(|n| format!("{}k:\n", " ".repeat(n)))
            .collect();
        let e = FileDescription::parse(&text).unwrap_err();
        assert!(e.contains("nested too deeply"), "{e}");
    });
}

#[test]
fn a_description_with_more_lines_than_replicas_allow_is_refused() {
    let text = "- x\n".repeat(2 * crate::limits::MAX_FILE_REPLICAS + 33);
    let e = FileDescription::parse(&text).unwrap_err();
    assert!(e.contains("too many lines"), "{e}");
}

#[test]
fn random_description_shapes_never_panic_or_overflow() {
    on_small_stack(|| {
        let pieces = [
            "- ",
            "-",
            "k: ",
            "k:",
            "a: b",
            "'q'",
            "\"q\\n\"",
            "[]",
            "{}",
            "\n",
            "\n  ",
            "\n    ",
            "  ",
            "x",
            ":",
            "replicas:",
            "chunks:",
            "server: s",
            "# c",
            "---",
        ];
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for round in 0..3000 {
            let len = 1 + next() % if round % 10 == 0 { 4000 } else { 60 };
            let text: String = (0..len)
                .map(|_| pieces[(next() % pieces.len() as u64) as usize])
                .collect();
            let _ = FileDescription::parse(&text);
        }
        let long = format!("k: {}", "a".repeat(1 << 20));
        let _ = FileDescription::parse(&long);
    });
}
