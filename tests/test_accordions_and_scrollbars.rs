//! Tests for independent accordion expand/collapse and thin scrollbar styling.
#![cfg(feature = "desktop")]

use marvelous_cloth::atelier::css_runner::{
    ACT_RUN_ACC_0, ACT_RUN_ACC_1, ACT_RUN_ACC_2, ACT_RUN_ACC_3, ACT_RUN_ACC_4, ACT_RUN_ACC_5,
};

#[test]
fn accordion_actions_toggle_independent_bits() {
    assert_ne!(ACT_RUN_ACC_0, ACT_RUN_ACC_1);
    assert_ne!(ACT_RUN_ACC_1, ACT_RUN_ACC_2);
    assert_ne!(ACT_RUN_ACC_2, ACT_RUN_ACC_3);
    assert_ne!(ACT_RUN_ACC_3, ACT_RUN_ACC_4);
    assert_ne!(ACT_RUN_ACC_4, ACT_RUN_ACC_5);

    // Each action must be an isolated single bit
    for act in [
        ACT_RUN_ACC_0,
        ACT_RUN_ACC_1,
        ACT_RUN_ACC_2,
        ACT_RUN_ACC_3,
        ACT_RUN_ACC_4,
        ACT_RUN_ACC_5,
    ] {
        assert_eq!(act.count_ones(), 1, "action {act} must be a single bit");
    }
}

#[test]
fn studio_v2_css_defines_thin_scrollbars_and_collapsed_accordions() {
    let css = include_str!("../src/atelier/css_ui/studio_v2.css");

    // Must define thin scrollbar rules
    assert!(
        css.contains("scrollbar-width: thin;"),
        "studio_v2.css must set scrollbar-width: thin"
    );
    assert!(
        css.contains("scrollbar-color:"),
        "studio_v2.css must define subtle scrollbar-color"
    );

    // Must define collapsed accordion rules
    assert!(
        css.contains(".accordion.collapsed"),
        "studio_v2.css must define .accordion.collapsed rule"
    );
    assert!(
        css.contains(".accordion.collapsed .accordion-content") || css.contains("display: none"),
        "studio_v2.css must hide collapsed accordion content"
    );

    // Must provide container padding for right gutter breathing room
    assert!(
        css.contains(".accordions-container"),
        "studio_v2.css must define .accordions-container"
    );
    assert!(
        css.contains(".drawer-content"),
        "studio_v2.css must define .drawer-content"
    );

    // Must define day-night pill switch styles
    assert!(
        css.contains(".day-night-switch"),
        "studio_v2.css must define .day-night-switch"
    );
    assert!(
        css.contains(".day-night-switch.dark"),
        "studio_v2.css must define dark switch state"
    );
    assert!(
        css.contains(".day-night-switch.light"),
        "studio_v2.css must define light switch state"
    );
    assert!(
        css.contains(".switch-thumb"),
        "studio_v2.css must define .switch-thumb"
    );
    // The same shapes remain mounted while their positions and colors animate.
    for class in [
        ".crater1", ".crater2", ".crater3", ".shape1", ".shape2", ".shape3", ".shape4", ".shape5",
    ] {
        assert!(css.contains(class), "studio_v2.css must define {class}");
    }

    // Must NOT contain AM / PM text labels
    assert!(
        !css.contains("'AM'"),
        "studio_v2.css must not contain 'AM' text label"
    );
    assert!(
        !css.contains("'PM'"),
        "studio_v2.css must not contain 'PM' text label"
    );

    // Must define command palette item icon styling
    assert!(
        css.contains(".palette-item .item-left"),
        "studio_v2.css must define .palette-item .item-left"
    );
    assert!(
        css.contains(".palette-item .icon"),
        "studio_v2.css must define .palette-item .icon"
    );
}

#[test]
fn day_night_switch_keeps_thumb_craters_and_sky_shapes_in_both_modes() {
    use cssimpler::core::Node;
    use marvelous_cloth::atelier::css_ui::icons::icon_day_night_switch;

    let dark_node = icon_day_night_switch(true);
    let light_node = icon_day_night_switch(false);

    let check_switch = |node: &Node, expected_mode: &str| {
        let Node::Element(elem) = node else {
            panic!("switch root must be an ElementNode");
        };
        assert!(
            elem.classes.iter().any(|c| c == expected_mode),
            "switch must have {expected_mode} class"
        );
        assert!(
            elem.classes.iter().any(|c| c == "day-night-switch"),
            "switch must have day-night-switch class"
        );

        // Handler knob
        let handler_node = elem
            .children
            .iter()
            .find(|child| {
                if let Node::Element(e) = child {
                    e.classes.iter().any(|c| c == "switch-thumb")
                } else {
                    false
                }
            })
            .expect("switch must contain handler knob");

        let Node::Element(handler) = handler_node else {
            panic!("handler must be Element");
        };
        assert_eq!(handler.children.len(), 3, "handler must contain 3 craters");
        for i in 1..=3 {
            let crater_class = format!("crater{i}");
            assert!(
                handler.children.iter().any(|child| {
                    if let Node::Element(e) = child {
                        e.classes.iter().any(|c| c == &crater_class)
                    } else {
                        false
                    }
                }),
                "handler must contain {crater_class}"
            );
        }

        // Five persistent shapes animate between clouds and stars.
        for i in 1..=5 {
            let star_class = format!("shape{i}");
            assert!(
                elem.children.iter().any(|child| {
                    if let Node::Element(e) = child {
                        e.classes.iter().any(|c| c == &star_class)
                    } else {
                        false
                    }
                }),
                "switch must contain {star_class}"
            );
        }
    };

    check_switch(&dark_node, "dark");
    check_switch(&light_node, "light");
}

#[test]
fn piece_selection_actions_are_isolated_bits() {
    use marvelous_cloth::atelier::css_runner::{
        ACT_RUN_PIECE_0, ACT_RUN_PIECE_1, ACT_RUN_PIECE_2, ACT_RUN_PIECE_3,
    };

    assert_ne!(ACT_RUN_PIECE_0, ACT_RUN_PIECE_1);
    assert_ne!(ACT_RUN_PIECE_1, ACT_RUN_PIECE_2);
    assert_ne!(ACT_RUN_PIECE_2, ACT_RUN_PIECE_3);

    for act in [
        ACT_RUN_PIECE_0,
        ACT_RUN_PIECE_1,
        ACT_RUN_PIECE_2,
        ACT_RUN_PIECE_3,
    ] {
        assert_eq!(
            act.count_ones(),
            1,
            "piece action {act} must be a single bit"
        );
    }
}

#[test]
fn solver_selection_actions_are_independent_of_accordions_and_pieces() {
    use marvelous_cloth::atelier::css_runner::{
        ACT_RUN_ACCORDION_TOGGLE, ACT_RUN_PIECE_0, ACT_RUN_PIECE_1, ACT_RUN_PIECE_2,
        ACT_RUN_PIECE_3, ACT_RUN_SOLVER_AL, ACT_RUN_SOLVER_CPU, ACT_RUN_SOLVER_GPU,
        ACT_RUN_SOLVER_MENU,
    };
    let accordions = ACT_RUN_ACC_0
        | ACT_RUN_ACC_1
        | ACT_RUN_ACC_2
        | ACT_RUN_ACC_3
        | ACT_RUN_ACC_4
        | ACT_RUN_ACC_5
        | ACT_RUN_ACCORDION_TOGGLE;
    let pieces = ACT_RUN_PIECE_0 | ACT_RUN_PIECE_1 | ACT_RUN_PIECE_2 | ACT_RUN_PIECE_3;
    let solvers = ACT_RUN_SOLVER_MENU | ACT_RUN_SOLVER_GPU | ACT_RUN_SOLVER_CPU | ACT_RUN_SOLVER_AL;
    assert_eq!(
        accordions & solvers,
        0,
        "opening an inspector must not switch the active solver"
    );
    assert_eq!(
        pieces & solvers,
        0,
        "selecting a pattern piece must not switch the active solver"
    );
    assert_eq!(
        accordions & pieces,
        0,
        "selecting a pattern piece must not toggle an inspector"
    );
    assert_eq!(solvers.count_ones(), 4);
    assert_eq!(pieces.count_ones(), 4);
}
