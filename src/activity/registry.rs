use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    ActivityAction, ActivityActionEvent, ActivityContent, ActivityError, ActivityId,
    ActivitySummary, CapacityKind, IdKind, MAX_ACTIVITIES, MAX_ACTIVITIES_PER_OWNER, MAX_OWNERS,
    OwnerId,
};

static OWNER_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static ACTIVITY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// A live activity. Its identity and owner survive full content replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    owner: OwnerId,
    id: ActivityId,
    content: ActivityContent,
}

impl Activity {
    pub fn owner(&self) -> OwnerId {
        self.owner
    }
    pub fn id(&self) -> ActivityId {
        self.id
    }
    pub fn content(&self) -> &ActivityContent {
        &self.content
    }
    pub fn supported_actions(&self) -> &'static [ActivityAction] {
        self.content.supported_actions()
    }
    pub fn summary(&self) -> ActivitySummary<'_> {
        self.content.summary()
    }
}

/// Bounded synchronous GUI state, with no callbacks, I/O, or runtime dependency.
/// Cloning copies state; it does not create a shared mutable registry.
///
/// The first start selects itself; later starts and replacements preserve focus.
/// Removing the selection chooses the oldest remaining activity (lowest ID).
/// Iteration follows that same deterministic order. There are no auto-evictions.
#[derive(Debug, Clone, Default)]
pub struct ActivityRegistry {
    owners: BTreeSet<OwnerId>,
    activities: BTreeMap<ActivityId, Activity>,
    selected: Option<ActivityId>,
}

impl ActivityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_owner(&mut self) -> Result<OwnerId, ActivityError> {
        capacity(self.owners.len(), MAX_OWNERS, CapacityKind::Owners)?;
        let owner = OwnerId(allocate(&OWNER_SEQUENCE, IdKind::Owner)?);
        self.owners.insert(owner);
        Ok(owner)
    }

    pub fn start(
        &mut self,
        owner: OwnerId,
        content: impl Into<ActivityContent>,
    ) -> Result<ActivityId, ActivityError> {
        self.require_owner(owner)?;
        capacity(
            self.activities.len(),
            MAX_ACTIVITIES,
            CapacityKind::Activities,
        )?;
        let count = self
            .activities
            .values()
            .filter(|activity| activity.owner == owner)
            .count();
        capacity(
            count,
            MAX_ACTIVITIES_PER_OWNER,
            CapacityKind::OwnerActivities(owner),
        )?;
        let id = ActivityId(allocate(&ACTIVITY_SEQUENCE, IdKind::Activity)?);
        self.activities.insert(
            id,
            Activity {
                owner,
                id,
                content: content.into(),
            },
        );
        self.selected.get_or_insert(id);
        Ok(id)
    }

    pub fn replace(
        &mut self,
        owner: OwnerId,
        id: ActivityId,
        content: impl Into<ActivityContent>,
    ) -> Result<(), ActivityError> {
        self.require_owned(owner, id)?;
        // Ownership validation above guarantees the entry exists.
        self.activities
            .get_mut(&id)
            .expect("validated activity")
            .content = content.into();
        Ok(())
    }

    pub fn remove(&mut self, owner: OwnerId, id: ActivityId) -> Result<Activity, ActivityError> {
        self.require_owned(owner, id)?;
        let removed = self.activities.remove(&id).expect("validated activity");
        self.reconcile_selection();
        Ok(removed)
    }

    /// Remove an owner and all its activities atomically, returning their count.
    pub fn remove_owner(&mut self, owner: OwnerId) -> Result<usize, ActivityError> {
        self.require_owner(owner)?;
        self.owners.remove(&owner);
        let before = self.activities.len();
        self.activities
            .retain(|_, activity| activity.owner != owner);
        self.reconcile_selection();
        Ok(before - self.activities.len())
    }

    pub fn select(&mut self, id: ActivityId) -> Result<(), ActivityError> {
        self.require_activity(id)?;
        self.selected = Some(id);
        Ok(())
    }

    pub fn selected(&self) -> Option<ActivityId> {
        self.selected
    }
    pub fn get(&self, id: ActivityId) -> Option<&Activity> {
        self.activities.get(&id)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Activity> + DoubleEndedIterator {
        self.activities.values()
    }

    /// Validate against the current snapshot, without changing it or selection.
    /// Delivery and subsequent producer updates are the caller's responsibility.
    pub fn route_action(
        &self,
        id: ActivityId,
        action: ActivityAction,
    ) -> Result<ActivityActionEvent, ActivityError> {
        let activity = self.require_activity(id)?;
        if !activity.supported_actions().contains(&action) {
            return Err(ActivityError::UnsupportedAction { id, action });
        }
        Ok(ActivityActionEvent {
            owner: activity.owner,
            id,
            action,
        })
    }

    fn require_owner(&self, owner: OwnerId) -> Result<(), ActivityError> {
        if !self.owners.contains(&owner) {
            return Err(ActivityError::UnknownOwner(owner));
        }
        Ok(())
    }

    fn require_activity(&self, id: ActivityId) -> Result<&Activity, ActivityError> {
        self.activities
            .get(&id)
            .ok_or(ActivityError::UnknownActivity(id))
    }

    fn require_owned(&self, owner: OwnerId, id: ActivityId) -> Result<(), ActivityError> {
        self.require_owner(owner)?;
        let activity = self.require_activity(id)?;
        if activity.owner != owner {
            return Err(ActivityError::Ownership {
                owner,
                id,
                actual_owner: activity.owner,
            });
        }
        Ok(())
    }

    fn reconcile_selection(&mut self) {
        if self
            .selected
            .is_some_and(|id| self.activities.contains_key(&id))
        {
            return;
        }
        self.selected = self.activities.keys().next().copied();
    }
}

fn capacity(count: usize, limit: usize, kind: CapacityKind) -> Result<(), ActivityError> {
    if count >= limit {
        return Err(ActivityError::Capacity { kind, limit });
    }
    Ok(())
}

fn allocate(sequence: &AtomicU64, kind: IdKind) -> Result<u64, ActivityError> {
    sequence
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |last| {
            last.checked_add(1)
        })
        .map(|last| last + 1)
        .map_err(|_| ActivityError::IdExhausted(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_ids_do_not_wrap() {
        let sequence = AtomicU64::new(u64::MAX - 1);
        assert_eq!(allocate(&sequence, IdKind::Owner), Ok(u64::MAX));
        assert_eq!(
            allocate(&sequence, IdKind::Owner),
            Err(ActivityError::IdExhausted(IdKind::Owner))
        );
        assert_eq!(sequence.load(Ordering::Relaxed), u64::MAX);
    }
}
