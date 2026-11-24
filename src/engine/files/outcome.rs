//! What a transfer posts back, however it ran: progress in 5% steps, and the descriptions an upload made (Subscriber.hs `SFPROG`, `SFDONE`).
use crossbeam_channel::Sender;

use super::transfers::{FileDone, FileKey, Sent, SentToMembers};
use crate::engine::jobs::JobDone;
use crate::xftp::Uploaded;
use crate::xftp::description::encode_description;

/// A direct file is made for this many recipients, one of which is used (`roundedFDCount 1`).
const RECIPIENTS: usize = 4;
const STEP: u8 = 5;

/// Internal.hs `roundedFDCount`: the next power of two, at least 4.
fn rounded_fd_count(n: usize) -> usize {
    n.max(1).next_power_of_two().max(RECIPIENTS)
}

/// The recipients an upload is made for: a friend's, or each of a group's members.
pub(super) fn recipients_for(members: Option<usize>) -> usize {
    members.map_or(RECIPIENTS, rounded_fd_count)
}

fn percent(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    u8::try_from(done.min(total) * 100 / total).unwrap_or(100)
}

/// Posts progress when it moved a step further than the last one posted, and the end.
pub(super) struct Throttle {
    key: FileKey,
    job: u64,
    last: u8,
}

impl Throttle {
    pub(super) fn new(key: FileKey, job: u64) -> Self {
        Self { key, job, last: 0 }
    }

    pub(super) fn tell(&mut self, done: u64, total: u64, tx: &Sender<JobDone>) {
        let p = percent(done, total);
        if p >= self.last.saturating_add(STEP) || (p == 100 && self.last < 100) {
            self.last = p;
            let (key, job) = (self.key, self.job);
            let progress = FileDone::Progress {
                key,
                job,
                percent: p,
            };
            let _ = tx.send(JobDone::File(progress)); // the engine is gone
        }
    }
}

/// An upload's result for the engine: the friend's one description, or one per member
/// (Subscriber.hs `SFDONE`).
pub(super) fn upload_done(
    key: FileKey,
    job: u64,
    members: Option<usize>,
    up: Result<Uploaded, String>,
) -> FileDone {
    let all = up.map(|u| {
        let all: Vec<String> = u.recipients.iter().map(encode_description).collect();
        (all, u.expires)
    });
    match members {
        None => {
            let result = all.and_then(|(all, expires)| {
                let description =
                    (all.into_iter().next()).ok_or("no description for the friend")?;
                Ok(Sent {
                    description,
                    expires,
                })
            });
            FileDone::Uploaded { key, job, result }
        }
        Some(n) => {
            let result = all.and_then(|(mut all, expires)| {
                if all.len() < n {
                    return Err("not enough file descriptions for the members".to_owned());
                }
                all.truncate(n);
                Ok(SentToMembers {
                    descriptions: all,
                    expires,
                })
            });
            FileDone::UploadedForMembers { key, job, result }
        }
    }
}

#[cfg(test)]
mod tests {
    use crossbeam_channel::unbounded;

    use super::*;

    #[test]
    fn a_groups_file_is_made_for_a_power_of_two_of_at_least_four_members() {
        let counts: Vec<usize> = [0, 1, 4, 5, 9].into_iter().map(rounded_fd_count).collect();
        assert_eq!(counts, [4, 4, 4, 8, 16]);
        assert_eq!(recipients_for(None), 4);
    }

    #[test]
    fn progress_is_posted_in_steps_of_five_percent() {
        let (tx, rx) = unbounded();
        let key = FileKey { friend: 1, item: 2 };
        let mut progress = Throttle::new(key, 1);
        for done in 0..=100 {
            progress.tell(done, 100, &tx);
        }
        let seen: Vec<u8> = rx
            .try_iter()
            .filter_map(|j| match j {
                JobDone::File(FileDone::Progress { percent, .. }) => Some(percent),
                JobDone::File(_) | JobDone::Relay(_) => None,
            })
            .collect();
        assert_eq!(seen, (1..=20).map(|n| n * 5).collect::<Vec<u8>>());
    }

    #[test]
    fn the_end_is_posted_even_off_a_five_percent_step() {
        let (tx, rx) = unbounded();
        let mut progress = Throttle::new(FileKey { friend: 1, item: 2 }, 1);
        for done in [96, 100, 100] {
            progress.tell(done, 100, &tx);
        }
        assert_eq!(rx.try_iter().count(), 2, "96, then 100 once");
    }
}
