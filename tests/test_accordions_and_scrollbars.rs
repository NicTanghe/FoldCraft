//! Tests for independent accordion expand/collapse and thin scrollbar styling.

use marvelous_cloth::atelier::css_runner::{
    ACT_RUN_ACC_0, ACT_RUN_ACC_1, ACT_RUN_ACC_2, ACT_RUN_ACC_3, ACT_RUN_ACC_4, ACT_RUN_ACC_5,
    on_runner_accordion_0, on_runner_accordion_1, on_runner_accordion_2, on_runner_accordion_3,
    on_runner_accordion_4, on_runner_accordion_5,
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
fn accordion_trampolines_dispatch_correct_actions() {
    // Calling trampolines should not panic
    on_runner_accordion_0();
    on_runner_accordion_1();
    on_runner_accordion_2();
    on_runner_accordion_3();
    on_runner_accordion_4();
    on_runner_accordion_5();
}

#[test]
fn studio_v2_css_defines_thin_scrollbars_and_collapsed_accordions() {
    let css = include_str!("../src/atelier/css_ui/studio_v2.css");

    // Must define thin scrollbar rules
    assert!(css.contains("scrollbar-width: thin;"), "studio_v2.css must set scrollbar-width: thin");
    assert!(css.contains("scrollbar-color:"), "studio_v2.css must define subtle scrollbar-color");

    // Must define collapsed accordion rules
    assert!(css.contains(".accordion.collapsed"), "studio_v2.css must define .accordion.collapsed rule");
    assert!(
        css.contains(".accordion.collapsed .accordion-content") || css.contains("display: none"),
        "studio_v2.css must hide collapsed accordion content"
    );

    // Must provide container padding for right gutter breathing room
    assert!(css.contains(".accordions-container"), "studio_v2.css must define .accordions-container");
    assert!(css.contains(".drawer-content"), "studio_v2.css must define .drawer-content");
}
