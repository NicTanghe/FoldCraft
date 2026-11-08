use glam::DVec2;
use marvelous_cloth::viewport_controller::{
    BoardNavigation, BoardView, InputOwnership, LogicalViewport, MAX_BOARD_SCALE, MIN_BOARD_SCALE,
    Modifiers, NavigationEvent, NavigationResult, PointerButton,
};

fn viewport() -> LogicalViewport {
    LogicalViewport::new(DVec2::new(100., 50.), DVec2::new(800., 600.)).unwrap()
}

fn near(actual: DVec2, expected: DVec2) {
    assert!(
        actual.distance(expected) < 1e-10,
        "actual={actual:?}, expected={expected:?}"
    );
}

fn press(id: u64, position: DVec2) -> NavigationEvent {
    NavigationEvent::PointerPressed {
        id,
        position,
        button: PointerButton::Middle,
        modifiers: Modifiers::default(),
    }
}

fn wheel(position: DVec2, delta_y: f64) -> NavigationEvent {
    NavigationEvent::Wheel {
        position,
        delta_y,
        modifiers: Modifiers::default(),
    }
}

#[derive(Default)]
struct Board {
    navigation: BoardNavigation,
    view: BoardView,
}

impl Board {
    fn event(&mut self, event: NavigationEvent) -> NavigationResult {
        self.navigation.handle(
            &mut self.view,
            Some(viewport()),
            InputOwnership::default(),
            event,
        )
    }
}

#[test]
fn viewport_rejects_invalid_dimensions_and_has_unambiguous_edges() {
    for size in [
        DVec2::ZERO,
        DVec2::new(-1., 1.),
        DVec2::new(1., 0.),
        DVec2::splat(f64::NAN),
        DVec2::splat(f64::INFINITY),
    ] {
        assert!(LogicalViewport::new(DVec2::ZERO, size).is_none());
    }
    assert!(LogicalViewport::new(DVec2::splat(f64::NAN), DVec2::ONE).is_none());
    assert!(LogicalViewport::new(DVec2::splat(f64::MAX), DVec2::splat(f64::MAX)).is_none());
    // A positive size can still collapse at an unrepresentably large origin.
    assert!(LogicalViewport::new(DVec2::splat(f64::MAX), DVec2::ONE).is_none());
    let rect = viewport();
    assert_eq!(rect.origin(), DVec2::new(100., 50.));
    assert_eq!(rect.size(), DVec2::new(800., 600.));
    assert!(rect.contains(rect.origin()));
    assert!(!rect.contains(rect.origin() + rect.size()));
    assert!(!rect.contains(DVec2::new(900., 60.)));
    assert!(!rect.contains(DVec2::splat(f64::NAN)));
}

#[test]
fn metric_transform_matches_rotated_board_convention_and_round_trips() {
    for rotation in [0., 0.37, std::f64::consts::FRAC_PI_2, -2.1] {
        let center = DVec2::new(1.2, -0.7);
        let view = BoardView::new(center, 600., rotation).unwrap();
        near(
            view.board_to_logical(viewport(), center).unwrap(),
            viewport().center(),
        );
        let (sin, cos) = rotation.sin_cos();
        near(
            view.board_to_logical(viewport(), center + DVec2::X)
                .unwrap(),
            viewport().center() + DVec2::new(cos, sin) * 600.,
        );
        for metres in [center, DVec2::new(-2., 5.), DVec2::new(0.02, 0.003)] {
            let logical = view.board_to_logical(viewport(), metres).unwrap();
            near(view.logical_to_board(viewport(), logical).unwrap(), metres);
        }
        assert_eq!(view.rotation(), rotation);
    }
}

#[test]
fn invalid_view_or_transform_inputs_do_not_create_nonfinite_coordinates() {
    for scale in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(BoardView::new(DVec2::ZERO, scale, 0.).is_none());
    }
    assert!(BoardView::new(DVec2::splat(f64::NAN), 300., 0.).is_none());
    assert!(BoardView::new(DVec2::ZERO, 300., f64::INFINITY).is_none());
    let mut view = BoardView::default();
    let original = view;
    assert!(!view.pan_by(DVec2::splat(f64::NAN)));
    assert!(
        view.logical_to_board(viewport(), DVec2::splat(f64::INFINITY))
            .is_none()
    );
    assert!(
        view.board_to_logical(viewport(), DVec2::splat(f64::MAX))
            .is_none()
    );
    for factor in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(!view.zoom_at(viewport(), viewport().center(), factor));
        assert_eq!(view, original);
    }
    assert_eq!(
        BoardView::new(DVec2::ZERO, 0.1, 0.).unwrap().scale(),
        MIN_BOARD_SCALE
    );
    assert_eq!(
        BoardView::new(DVec2::ZERO, 1e9, 0.).unwrap().scale(),
        MAX_BOARD_SCALE
    );
}

#[test]
fn zoom_keeps_cursor_anchor_including_both_scale_limits() {
    let cursor = DVec2::new(170., 590.);
    let mut view = BoardView::new(DVec2::new(1.2, -2.), 300., 0.73).unwrap();
    let anchor = view.logical_to_board(viewport(), cursor).unwrap();
    for (factor, scale) in [
        (2., 600.),
        (f64::MAX, MAX_BOARD_SCALE),
        (1e-300, MIN_BOARD_SCALE),
    ] {
        assert!(view.zoom_at(viewport(), cursor, factor));
        assert_eq!(view.scale(), scale);
        near(view.logical_to_board(viewport(), cursor).unwrap(), anchor);
    }
    let at_limit = view;
    assert!(!view.zoom_at(viewport(), cursor, 0.01));
    assert_eq!(view, at_limit);
}

#[test]
fn captured_pan_follows_pointer_outside_and_applies_final_release_position() {
    let mut board = Board {
        view: BoardView::new(DVec2::new(0.1, -0.2), 600., 0.9).unwrap(),
        ..Default::default()
    };
    let start = DVec2::new(240., 150.);
    let anchor = board.view.logical_to_board(viewport(), start).unwrap();
    let started = board.event(press(7, start));
    assert_eq!(
        started,
        NavigationResult {
            consumed: true,
            view_changed: false,
            capture_changed: true
        }
    );
    assert_eq!(board.navigation.captured_pointer(), Some(7));
    let outside = DVec2::new(-400., 1200.);
    assert!(
        board
            .event(NavigationEvent::PointerMoved {
                id: 7,
                position: outside
            })
            .view_changed
    );
    near(
        board.view.board_to_logical(viewport(), anchor).unwrap(),
        outside,
    );
    let end = DVec2::new(-410., 1215.);
    let ended = board.event(NavigationEvent::PointerReleased {
        id: 7,
        position: end,
        button: PointerButton::Middle,
    });
    assert_eq!(
        ended,
        NavigationResult {
            consumed: true,
            view_changed: true,
            capture_changed: true
        }
    );
    near(
        board.view.board_to_logical(viewport(), anchor).unwrap(),
        end,
    );
    assert_eq!(board.navigation.captured_pointer(), None);
    let final_view = board.view;
    assert!(
        !board
            .event(NavigationEvent::PointerMoved {
                id: 7,
                position: start
            })
            .consumed
    );
    assert_eq!(board.view, final_view);
}

#[test]
fn capture_ignores_other_pointers_buttons_and_wheel() {
    let mut board = Board::default();
    let start = viewport().center();
    board.event(press(1, start));
    let original = board.view;
    for event in [
        press(2, start),
        NavigationEvent::PointerMoved {
            id: 2,
            position: start + DVec2::splat(40.),
        },
        NavigationEvent::PointerReleased {
            id: 2,
            position: start,
            button: PointerButton::Middle,
        },
        NavigationEvent::PointerReleased {
            id: 1,
            position: start,
            button: PointerButton::Primary,
        },
        wheel(start, 100.),
    ] {
        assert!(!board.event(event).consumed);
        assert_eq!(board.navigation.captured_pointer(), Some(1));
        assert_eq!(board.view, original);
    }
}

#[test]
fn modified_gestures_and_context_buttons_remain_host_owned() {
    let mut board = Board::default();
    for modifiers in [
        Modifiers {
            shift: true,
            ..Default::default()
        },
        Modifiers {
            control: true,
            ..Default::default()
        },
        Modifiers {
            alt: true,
            ..Default::default()
        },
        Modifiers {
            command: true,
            ..Default::default()
        },
    ] {
        for event in [
            NavigationEvent::PointerPressed {
                id: 1,
                position: viewport().center(),
                button: PointerButton::Middle,
                modifiers,
            },
            NavigationEvent::Wheel {
                position: viewport().center(),
                delta_y: 100.,
                modifiers,
            },
        ] {
            assert!(!board.event(event).consumed);
        }
    }
    for button in [PointerButton::Primary, PointerButton::Secondary] {
        assert!(
            !board
                .event(NavigationEvent::PointerPressed {
                    id: 1,
                    position: viewport().center(),
                    button,
                    modifiers: Modifiers::default()
                })
                .consumed
        );
    }
    assert!(!board.event(press(1, DVec2::ZERO)).consumed);
    assert_eq!(board.navigation.captured_pointer(), None);
    assert_eq!(board.view, BoardView::default());
}

#[test]
fn wheel_uses_logical_pixels_and_saturates_extreme_finite_deltas() {
    let mut board = Board::default();
    let cursor = DVec2::new(320., 230.);
    let anchor = board.view.logical_to_board(viewport(), cursor).unwrap();
    assert!(board.event(wheel(cursor, 100.)).view_changed);
    assert!((board.view.scale() - 300. * 0.2_f64.exp()).abs() < 1e-10);
    assert!(board.event(wheel(cursor, f64::MAX)).view_changed);
    assert_eq!(board.view.scale(), MAX_BOARD_SCALE);
    near(
        board.view.logical_to_board(viewport(), cursor).unwrap(),
        anchor,
    );
    let saturated = board.view;
    let result = board.event(wheel(cursor, f64::MAX));
    assert!(result.consumed);
    assert!(!result.view_changed);
    assert_eq!(board.view, saturated);
    assert!(board.event(wheel(cursor, -f64::MAX)).view_changed);
    assert_eq!(board.view.scale(), MIN_BOARD_SCALE);
    near(
        board.view.logical_to_board(viewport(), cursor).unwrap(),
        anchor,
    );
    for event in [
        wheel(cursor, 0.),
        wheel(cursor, f64::NAN),
        wheel(cursor, f64::INFINITY),
        wheel(DVec2::ZERO, 100.),
    ] {
        assert!(!board.event(event).consumed);
    }
}

#[test]
fn focus_pointer_ownership_or_viewport_changes_release_capture_without_jump() {
    for (rect, ownership) in [
        (
            Some(viewport()),
            InputOwnership {
                window_focused: false,
                ..Default::default()
            },
        ),
        (
            Some(viewport()),
            InputOwnership {
                pointer_available: false,
                ..Default::default()
            },
        ),
        (None, InputOwnership::default()),
        (
            LogicalViewport::new(DVec2::ZERO, DVec2::splat(500.)),
            InputOwnership::default(),
        ),
    ] {
        let mut board = Board::default();
        board.event(press(1, viewport().center()));
        board.event(NavigationEvent::PointerMoved {
            id: 1,
            position: viewport().center() + DVec2::splat(30.),
        });
        let last_view = board.view;
        let result = board.navigation.handle(
            &mut board.view,
            rect,
            ownership,
            NavigationEvent::OwnershipChanged,
        );
        assert!(result.capture_changed && result.consumed && !result.view_changed);
        assert_eq!(board.navigation.captured_pointer(), None);
        assert_eq!(board.view, last_view);
        assert!(
            !board
                .event(NavigationEvent::PointerMoved {
                    id: 1,
                    position: viewport().center()
                })
                .consumed
        );
        assert_eq!(board.view, last_view);
    }
}

#[test]
fn text_field_owns_escape_but_not_an_existing_pointer_capture() {
    let mut board = Board::default();
    board.event(press(1, viewport().center()));
    let ownership = InputOwnership {
        keyboard_available: false,
        ..Default::default()
    };
    let result = board.navigation.handle(
        &mut board.view,
        Some(viewport()),
        ownership,
        NavigationEvent::Escape,
    );
    assert!(!result.consumed);
    assert_eq!(board.navigation.captured_pointer(), Some(1));
    assert!(board.event(NavigationEvent::Escape).capture_changed);
    board.event(press(2, viewport().center()));
    assert!(
        board
            .navigation
            .handle(
                &mut board.view,
                Some(viewport()),
                ownership,
                NavigationEvent::Cancel
            )
            .capture_changed
    );
    assert_eq!(board.navigation.captured_pointer(), None);
}

#[test]
fn invalid_pointer_samples_do_not_poison_pan_and_invalid_release_ends_it() {
    let mut board = Board::default();
    let start = viewport().center();
    board.event(press(1, start));
    let original = board.view;
    let result = board.event(NavigationEvent::PointerMoved {
        id: 1,
        position: DVec2::splat(f64::NAN),
    });
    assert!(result.consumed && !result.view_changed);
    assert_eq!(board.view, original);
    board.event(NavigationEvent::PointerMoved {
        id: 1,
        position: start + DVec2::new(30., 60.),
    });
    near(board.view.center_metres(), DVec2::new(-0.1, -0.2));
    let last_view = board.view;
    assert!(
        board
            .event(NavigationEvent::PointerReleased {
                id: 1,
                position: DVec2::splat(f64::INFINITY),
                button: PointerButton::Middle
            })
            .capture_changed
    );
    assert_eq!(board.view, last_view);
    assert_eq!(board.navigation.captured_pointer(), None);
}
