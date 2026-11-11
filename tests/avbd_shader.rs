#[test]
fn resident_avbd_wgsl_validates_without_a_gpu() {
    let source = [
        include_str!("../src/gpu_avbd/main.wgsl"),
        include_str!("../src/gpu_avbd/jet.wgsl"),
        include_str!("../src/gpu_avbd/contact.wgsl"),
        include_str!("../src/gpu_avbd/solve.wgsl"),
        include_str!("../src/gpu_avbd/al.wgsl"),
    ]
    .join("\n");
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
}
