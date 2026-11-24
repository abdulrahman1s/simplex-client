//! An invitation into a group: join it or turn it down.

use super::Group;
use crate::client::Result;

#[derive(Clone, Debug)]
pub struct GroupInvite {
    group: Group,
}

impl GroupInvite {
    pub(in crate::client) const fn new(group: Group) -> Self {
        Self { group }
    }

    /// The group, as the invitation describes it.
    #[must_use]
    pub const fn group(&self) -> &Group {
        &self.group
    }

    pub fn accept(&self) -> Result<()> {
        self.group.client().join_group(self.group.id())
    }

    /// Declining is leaving before joining: the group stays listed as left.
    pub fn reject(&self) -> Result<()> {
        self.group.leave()
    }
}
