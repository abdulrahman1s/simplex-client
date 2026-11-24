//! Upstream's `tests/ValidNames.hs`, case for case.

use super::*;

#[test]
fn names_are_cleaned_as_upstream_mk_valid_name_does() {
    let cases = [
        ("alice", "alice"),
        (" alice", "alice"),
        ("?alice", "alice"),
        ("алиса", "алиса"),
        ("John Doe", "John Doe"),
        ("J.Doe", "JDoe"),
        ("J. Doe", "J Doe"),
        ("J..Doe", "JDoe"),
        ("J ..Doe", "J Doe"),
        ("J ... Doe", "J Doe"),
        ("J .... Doe", "J Doe"),
        ("J . . Doe", "J Doe"),
        ("@alice", "alice"),
        ("#alice", "alice"),
        ("'alice", "alice"),
        (" alice", "alice"),
        ("alice ", "alice"),
        ("John  Doe", "John Doe"),
        ("'John Doe'", "John Doe"),
        ("\"John Doe\"", "John Doe"),
        ("`John Doe`", "John Doe"),
        ("John \"Doe\"", "John Doe"),
        ("John `Doe`", "John Doe"),
        ("alice/bob", "alicebob"),
        ("alice / bob", "alice bob"),
        ("alice /// bob", "alice bob"),
        ("alice //// bob", "alice bob"),
        ("alice >>= bob", "alice >>= bob"),
        ("alice@example.com", "aliceexamplecom"),
        ("alice <> bob", "alice <> bob"),
        ("alice -> bob", "alice -> bob"),
        ("alice & bob", "alice & bob"),
        ("alice && bob", "alice & bob"),
        ("alice & & bob", "alice & bob"),
        ("alice-bob", "alice-bob"),
        ("alice--bob", "alice-bob"),
        ("alice -- bob", "alice - bob"),
        ("alice \\ bob", "alice bob"),
        ("alice (bob)", "alice bob"),
        ("alice: bob", "alice: bob"),
        ("alice 👍", "alice 👍"),
        ("👍", "👍"),
        ("alice >", "alice >"),
        ("> alice", "alice"),
        ("123", "123"),
        ("123 alice", "123 alice"),
        (
            "01234567890123456789012345678901234567890123456789extra",
            "01234567890123456789012345678901234567890123456789",
        ),
        (
            "0123456789012345678901234567890123456789012345678 extra",
            "0123456789012345678901234567890123456789012345678",
        ),
        ("alice_1", "alice-1"),
        ("alice_bob_12", "alice_bob-12"),
        ("alice _1", "alice -1"),
        ("alice_", "alice_"),
        ("alice_bob", "alice_bob"),
        ("alice_1b", "alice_1b"),
    ];
    for (given, want) in cases {
        assert_eq!(valid_name(given), want, "{given:?}");
    }
    assert_eq!(valid_name(&valid_name("alice_1")), "alice-1");
}

#[test]
fn a_name_is_valid_only_as_it_would_be_cleaned() {
    assert!(is_valid_name("Alice"));
    assert!(is_valid_name("алиса 👍"));
    assert!(!is_valid_name(""));
    assert!(!is_valid_name(" alice"));
    assert!(!is_valid_name("J.Doe"));
    assert!(!is_valid_name(&"a".repeat(MAX_NAME_CHARS + 1)));
    assert!(is_valid_name(&"ж".repeat(MAX_NAME_CHARS)));
}
