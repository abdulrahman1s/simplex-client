//! Files: the official client's size limit.

use super::*;
use crate::engine::files::MemFiles;
use crate::limits::MAX_FILE_SIZE;
use crate::types::ErrorCode;

#[test]
fn a_file_over_one_gib_is_refused_as_too_large() {
    let (mut t, _fake) = setup();
    let (bob, _) = t.friend("bob");
    let path = t.dir.0.join("files").join("big.bin");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_FILE_SIZE + 1).unwrap(); // sparse: nothing is written
    let sends = t.e.agent.sent().len();
    let out = send(&mut t, bob, path, FileKind::File);
    let code = out.iter().find_map(|e| match e {
        Event::Failed { code, .. } => Some(*code),
        _ => None,
    });
    assert_eq!(code, Some(ErrorCode::FileTooLarge));
    assert_eq!(t.e.agent.sent().len(), sends, "no offer goes out");
}

#[test]
fn the_memory_limit_never_passes_the_official_one() {
    let mem = MemFiles::new(10);
    mem.set_limit(u64::MAX);
    assert_eq!(mem.limit(), MAX_FILE_SIZE);
    let small = MemFiles::new(4);
    let refused = small.put("x", vec![0; 5], false).unwrap_err();
    assert_eq!(refused.code, ErrorCode::FileTooLarge);
}
