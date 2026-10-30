use std::time::Duration;

use super::*;

fn transfer(state: TransferState) -> Transfer {
    let completed = if state == TransferState::Completed {
        10
    } else {
        4
    };
    Transfer::new(
        "download",
        TransferDirection::Download,
        completed,
        Some(10),
        state,
    )
    .unwrap()
}

fn media(state: MediaState) -> Media {
    Media::new(
        "song",
        "artist",
        Duration::from_secs(3),
        Some(Duration::from_secs(10)),
        state,
    )
    .unwrap()
}

#[test]
fn owned_lifecycle_and_clone_independence() {
    fn gui_safe<T: Clone + Send + Sync>() {}
    gui_safe::<ActivityRegistry>();
    let mut registry = ActivityRegistry::new();
    let owner = registry.register_owner().unwrap();
    let title = String::from("owned title");
    let snapshot = Transfer::new(
        title.clone(),
        TransferDirection::Upload,
        1,
        None,
        TransferState::Running,
    )
    .unwrap();
    let id = registry.start(owner, snapshot).unwrap();
    drop(title);
    let mut cloned = registry.clone();
    registry
        .replace(owner, id, media(MediaState::Playing))
        .unwrap();
    assert!(matches!(
        registry.get(id).unwrap().content(),
        ActivityContent::Media(_)
    ));
    assert!(matches!(
        cloned.get(id).unwrap().summary(),
        ActivitySummary::Transfer {
            title: "owned title",
            direction: TransferDirection::Upload,
            percentage: None,
            ..
        }
    ));
    let second = cloned
        .start(owner, transfer(TransferState::Paused))
        .unwrap();
    let third = registry
        .start(owner, transfer(TransferState::Running))
        .unwrap();
    assert!(second > id && third > second);
    assert!(registry.get(second).is_none());
    assert!(cloned.get(third).is_none());
    assert_eq!(registry.remove(owner, id).unwrap().id(), id);
    assert!(cloned.get(id).is_some());
    assert_eq!(registry.remove_owner(owner), Ok(1));
    assert_eq!(registry.selected(), None);
    assert_eq!(registry.iter().len(), 0);
    assert_eq!(
        registry.start(owner, transfer(TransferState::Running)),
        Err(ActivityError::UnknownOwner(owner))
    );
    let next_owner = registry.register_owner().unwrap();
    assert!(next_owner > owner);
    let mut independent = ActivityRegistry::new();
    assert!(independent.register_owner().unwrap() > next_owner);
}

#[test]
fn selection_is_stable_and_fallback_is_oldest() {
    let mut r = ActivityRegistry::new();
    let a = r.register_owner().unwrap();
    let b = r.register_owner().unwrap();
    assert_eq!(r.selected(), None);
    let first = r.start(a, transfer(TransferState::Running)).unwrap();
    let second = r.start(b, media(MediaState::Playing)).unwrap();
    let third = r.start(a, transfer(TransferState::Paused)).unwrap();
    assert_eq!(r.selected(), Some(first));
    r.select(second).unwrap();
    r.replace(a, first, transfer(TransferState::Completed))
        .unwrap();
    r.replace(b, second, media(MediaState::Paused)).unwrap();
    let fourth = r.start(b, transfer(TransferState::Running)).unwrap();
    assert_eq!(r.selected(), Some(second));
    assert_eq!(
        r.iter().map(Activity::id).collect::<Vec<_>>(),
        [first, second, third, fourth]
    );
    r.remove(b, fourth).unwrap();
    assert_eq!(r.selected(), Some(second));
    r.remove_owner(b).unwrap();
    assert_eq!(r.selected(), Some(first));
    r.remove(a, first).unwrap();
    assert_eq!(r.selected(), Some(third));
    r.remove(a, third).unwrap();
    assert_eq!(r.selected(), None);
    assert_eq!(r.select(first), Err(ActivityError::UnknownActivity(first)));
    assert_eq!(r.selected(), None);
    let fresh = r.start(a, transfer(TransferState::Running)).unwrap();
    assert!(fresh > fourth);
    assert_eq!(r.selected(), Some(fresh));
}

#[test]
fn ownership_and_unknown_resources_have_context_and_no_side_effects() {
    let mut r = ActivityRegistry::new();
    let a = r.register_owner().unwrap();
    let b = r.register_owner().unwrap();
    let id = r.start(a, transfer(TransferState::Running)).unwrap();
    let expected = ActivityError::Ownership {
        owner: b,
        id,
        actual_owner: a,
    };
    assert_eq!(
        r.replace(b, id, media(MediaState::Playing)),
        Err(expected.clone())
    );
    assert_eq!(r.remove(b, id), Err(expected));
    assert_eq!(r.get(id).unwrap().owner(), a);
    assert!(matches!(
        r.get(id).unwrap().content(),
        ActivityContent::Transfer(_)
    ));
    r.remove_owner(b).unwrap();
    assert_eq!(
        r.replace(b, id, media(MediaState::Playing)),
        Err(ActivityError::UnknownOwner(b))
    );
    assert_eq!(r.remove_owner(b), Err(ActivityError::UnknownOwner(b)));
    assert_eq!(r.remove(b, id), Err(ActivityError::UnknownOwner(b)));
    r.remove(a, id).unwrap();
    assert_eq!(
        r.replace(a, id, media(MediaState::Playing)),
        Err(ActivityError::UnknownActivity(id))
    );
    assert_eq!(r.remove(a, id), Err(ActivityError::UnknownActivity(id)));
    assert_eq!(
        r.route_action(id, ActivityAction::Pause),
        Err(ActivityError::UnknownActivity(id))
    );
}

#[test]
fn owner_and_per_owner_quotas_recover_without_eviction() {
    let mut r = ActivityRegistry::new();
    let owners: Vec<_> = (0..MAX_OWNERS)
        .map(|_| r.register_owner().unwrap())
        .collect();
    assert_eq!(
        r.register_owner(),
        Err(ActivityError::Capacity {
            kind: CapacityKind::Owners,
            limit: MAX_OWNERS
        })
    );
    let owner = owners[0];
    let ids: Vec<_> = (0..MAX_ACTIVITIES_PER_OWNER)
        .map(|_| r.start(owner, transfer(TransferState::Running)).unwrap())
        .collect();
    assert_eq!(
        r.start(owner, transfer(TransferState::Running)),
        Err(ActivityError::Capacity {
            kind: CapacityKind::OwnerActivities(owner),
            limit: MAX_ACTIVITIES_PER_OWNER
        })
    );
    r.replace(owner, ids[0], media(MediaState::Playing))
        .unwrap();
    assert_eq!(r.iter().len(), MAX_ACTIVITIES_PER_OWNER);
    assert_eq!(r.selected(), Some(ids[0]));
    r.remove(owner, ids[1]).unwrap();
    assert!(r.start(owner, transfer(TransferState::Running)).unwrap() > *ids.last().unwrap());
    assert_eq!(r.remove_owner(owner), Ok(MAX_ACTIVITIES_PER_OWNER));
    assert!(r.register_owner().unwrap() > *owners.last().unwrap());
}

#[test]
fn total_quota_is_hard_and_replacement_is_allowed_at_capacity() {
    let mut r = ActivityRegistry::new();
    let owners: Vec<_> = (0..9).map(|_| r.register_owner().unwrap()).collect();
    for owner in &owners[..8] {
        for _ in 0..MAX_ACTIVITIES_PER_OWNER {
            r.start(*owner, transfer(TransferState::Running)).unwrap();
        }
    }
    let first = r.selected().unwrap();
    assert_eq!(r.iter().len(), MAX_ACTIVITIES);
    assert_eq!(
        r.start(owners[8], media(MediaState::Playing)),
        Err(ActivityError::Capacity {
            kind: CapacityKind::Activities,
            limit: MAX_ACTIVITIES
        })
    );
    r.replace(owners[0], first, media(MediaState::Paused))
        .unwrap();
    assert_eq!(r.selected(), Some(first));
    r.remove(owners[0], first).unwrap();
    r.start(owners[8], media(MediaState::Playing)).unwrap();
    assert_eq!(r.iter().len(), MAX_ACTIVITIES);
}

#[test]
fn utf8_byte_bounds_do_not_truncate_or_echo_text() {
    let title = "🦀".repeat(64);
    let supporting = "é".repeat(1024);
    let snapshot = Transfer::new(
        &title,
        TransferDirection::Download,
        0,
        None,
        TransferState::Running,
    )
    .unwrap()
    .with_supporting(&supporting)
    .unwrap();
    assert_eq!(snapshot.title(), title);
    assert_eq!(snapshot.supporting(), supporting);
    assert!(
        Media::new(
            &title,
            &supporting,
            Duration::ZERO,
            None,
            MediaState::Stopped
        )
        .is_ok()
    );
    let long_title = format!("{title}é");
    let error = Transfer::new(
        &long_title,
        TransferDirection::Upload,
        0,
        None,
        TransferState::Paused,
    )
    .unwrap_err();
    assert_eq!(
        error,
        ActivityError::InvalidField(InvalidField::TextTooLong {
            field: "title",
            bytes: 258,
            max: MAX_TITLE_BYTES
        })
    );
    assert!(!error.to_string().contains(&long_title));
    assert!(Media::new(long_title, "", Duration::ZERO, None, MediaState::Playing).is_err());
    let long_supporting = format!("{supporting}a");
    assert_eq!(
        snapshot.with_supporting(&long_supporting),
        Err(ActivityError::InvalidField(InvalidField::TextTooLong {
            field: "supporting",
            bytes: 2049,
            max: MAX_SUPPORTING_BYTES
        }))
    );
    assert_eq!(
        Media::new(
            "",
            long_supporting,
            Duration::ZERO,
            None,
            MediaState::Paused
        ),
        Err(ActivityError::InvalidField(InvalidField::TextTooLong {
            field: "artist",
            bytes: 2049,
            max: MAX_SUPPORTING_BYTES
        }))
    );
    assert!(
        Transfer::new(
            "",
            TransferDirection::Upload,
            0,
            None,
            TransferState::Running
        )
        .is_ok()
    );
}

#[test]
fn transfer_integer_edges_and_finite_presentation() {
    for state in [
        TransferState::Running,
        TransferState::Paused,
        TransferState::Completed,
    ] {
        let t = Transfer::new(
            "",
            TransferDirection::Download,
            u64::MAX,
            Some(u64::MAX),
            state,
        )
        .unwrap();
        assert_eq!(t.percentage(), Some(100.0));
        assert_eq!(
            Transfer::new("", TransferDirection::Upload, 0, Some(0), state)
                .unwrap()
                .percentage(),
            Some(if state == TransferState::Completed {
                100.0
            } else {
                0.0
            })
        );
        assert_eq!(
            Transfer::new("", TransferDirection::Download, u64::MAX, None, state)
                .unwrap()
                .percentage(),
            None
        );
    }
    assert_eq!(
        Transfer::new(
            "",
            TransferDirection::Download,
            1,
            Some(0),
            TransferState::Running
        ),
        Err(ActivityError::InvalidField(
            InvalidField::ProgressExceedsTotal {
                completed: 1,
                total: 0
            }
        ))
    );
    assert_eq!(
        Transfer::new(
            "",
            TransferDirection::Download,
            3,
            Some(4),
            TransferState::Completed
        ),
        Err(ActivityError::InvalidField(
            InvalidField::IncompleteTransfer {
                completed: 3,
                total: 4
            }
        ))
    );
    for completed in [0, 1, u64::MAX / 2, u64::MAX - 1, u64::MAX] {
        let percentage = Transfer::new(
            "",
            TransferDirection::Download,
            completed,
            Some(u64::MAX),
            TransferState::Running,
        )
        .unwrap()
        .percentage()
        .unwrap();
        assert!(percentage.is_finite() && (0.0..=100.0).contains(&percentage));
    }
    assert_eq!(transfer(TransferState::Running).percentage(), Some(40.0));
}

#[test]
fn media_duration_edges_and_summary_preserve_exact_position() {
    for state in [MediaState::Playing, MediaState::Paused, MediaState::Stopped] {
        for duration in [Duration::ZERO, Duration::MAX] {
            let m = Media::new("title", "artist", duration, Some(duration), state).unwrap();
            assert_eq!(m.position(), duration);
            assert_eq!(m.duration(), Some(duration));
        }
        assert!(Media::new("", "", Duration::MAX, None, state).is_ok());
        let position = Duration::from_nanos(1);
        assert_eq!(
            Media::new("", "", position, Some(Duration::ZERO), state),
            Err(ActivityError::InvalidField(
                InvalidField::PositionExceedsDuration {
                    position,
                    duration: Duration::ZERO
                }
            ))
        );
    }
    assert_eq!(
        ActivityContent::from(media(MediaState::Playing)).summary(),
        ActivitySummary::Media {
            title: "song",
            artist: "artist",
            position: Duration::from_secs(3),
            duration: Some(Duration::from_secs(10)),
            state: MediaState::Playing
        }
    );
}

#[test]
fn action_matrix_is_validated_against_current_snapshot_without_mutation() {
    use ActivityAction::*;
    let snapshots: Vec<(ActivityContent, &[ActivityAction])> = vec![
        (transfer(TransferState::Running).into(), &[Pause, Cancel]),
        (transfer(TransferState::Paused).into(), &[Resume, Cancel]),
        (transfer(TransferState::Completed).into(), &[]),
        (media(MediaState::Playing).into(), &[Pause, Previous, Next]),
        (media(MediaState::Paused).into(), &[Resume, Previous, Next]),
        (media(MediaState::Stopped).into(), &[Resume, Previous, Next]),
    ];
    let mut r = ActivityRegistry::new();
    let owner = r.register_owner().unwrap();
    let id = r.start(owner, transfer(TransferState::Running)).unwrap();
    for (snapshot, supported) in snapshots {
        r.replace(owner, id, snapshot.clone()).unwrap();
        assert_eq!(r.get(id).unwrap().supported_actions(), supported);
        for action in [Pause, Resume, Cancel, Previous, Next] {
            let result = r.route_action(id, action);
            if supported.contains(&action) {
                let event = result.unwrap();
                assert_eq!(
                    (event.owner(), event.id(), event.action()),
                    (owner, id, action)
                );
            } else {
                assert_eq!(result, Err(ActivityError::UnsupportedAction { id, action }));
            }
            assert_eq!(r.get(id).unwrap().content(), &snapshot);
            assert_eq!(r.selected(), Some(id));
        }
    }
}
