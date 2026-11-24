//! Opening again: the right key reads back, a wrong one is refused, ids stay used.
use super::{Bed, both};
use crate::store::StoreError;

fn reopen_with_the_right_key_reads_everything_back(mut d: Bed) {
    let (item, other);
    {
        let mut s = d.store("k1");
        s.put("agent", b"conn-1", b"state").unwrap();
        item = s.insert_item(b"direct:1", b"hello").unwrap();
        other = s.next_id("friends").unwrap();
        s.put_file(7, b"file record").unwrap();
        d.close(s);
    }
    let mut s = d.store("k1");
    assert_eq!(
        s.get("agent", b"conn-1").unwrap().as_deref(),
        Some(&b"state"[..])
    );
    assert_eq!(
        s.page(b"direct:1", None, 10).unwrap(),
        vec![(item, b"hello".to_vec())]
    );
    assert_eq!(s.next_id("friends").unwrap(), other + 1);
    assert_eq!(s.get_file(7).unwrap().as_deref(), Some(&b"file record"[..]));
    assert_eq!(s.get_file(8).unwrap(), None);
}

fn wrong_key_is_its_own_error(mut d: Bed) {
    let s = d.store("right");
    d.close(s);
    assert!(matches!(d.open("wrong"), Err(StoreError::WrongKey)));
    let s = d.store("right");
    d.close(s);
}

fn item_ids_stay_unused_after_a_reopen(mut d: Bed) {
    let mut s = d.store("k");
    let gone = s.insert_item(b"c", b"x").unwrap();
    s.delete_item(b"c", gone).unwrap();
    d.close(s);
    let mut s = d.store("k");
    assert!(s.insert_item(b"c", b"y").unwrap() > gone);
}

fn keys_with_prefix_lists_one_namespace_and_filters_on_the_real_key(d: Bed) {
    let mut s = d.store("k");
    for k in [&b"rcv/1"[..], b"rcv/2", b"snd/1"] {
        s.put("queues", k, b"v").unwrap();
    }
    s.put("other", b"rcv/9", b"v").unwrap();
    let mut keys = s.keys_with_prefix("queues", b"rcv/").unwrap();
    keys.sort();
    assert_eq!(keys, vec![b"rcv/1".to_vec(), b"rcv/2".to_vec()]);
    s.delete("queues", b"rcv/1").unwrap();
    assert_eq!(s.keys_with_prefix("queues", b"").unwrap().len(), 2);
}

fn every_committed_write_moves_the_revision(d: Bed) {
    let mut s = d.store("k");
    assert_eq!(s.revision(), 0);
    s.put("ns", b"a", b"1").unwrap();
    s.insert_item(b"c", b"x").unwrap();
    assert_eq!(s.revision(), 2);
    assert!(s.update_item(b"c", 99, b"x").is_err());
    assert_eq!(s.revision(), 2, "a rolled-back write changes nothing");
    s.get("ns", b"a").unwrap();
    assert_eq!(s.revision(), 2, "reads do not count");
}

both!(
    reopen_with_the_right_key_reads_everything_back,
    wrong_key_is_its_own_error,
    item_ids_stay_unused_after_a_reopen,
    keys_with_prefix_lists_one_namespace_and_filters_on_the_real_key,
    every_committed_write_moves_the_revision,
);

#[cfg(feature = "native")]
#[test]
fn a_file_that_is_not_a_database_is_corrupt() {
    let d = super::Dir::new();
    std::fs::write(d.db(), vec![7u8; 4096]).unwrap();
    assert!(matches!(
        crate::store::Store::open(&d.db(), "k"),
        Err(StoreError::Corrupt(_))
    ));
}
