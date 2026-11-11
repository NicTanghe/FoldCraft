// Independent finite AVBD / squared-distance IPC-style barrier pipeline.
// All iteration decisions stay on the device. No global floating-point atomics.
struct Term {ids:vec4u,info:vec4u,a:vec4f,b:vec4f,c:vec4f}
struct Particle {initial:vec4f, velocity_mass:vec4f,inertia_target:vec4f,delta:vec4f,energy:vec4f}
struct Node {lo:vec4f,hi:vec4f,info:vec4u}
struct Pair {ids:vec4u,kind:u32,pad0:u32,pad1:u32,pad2:u32,next:vec4u}
struct PairBuffer {count:atomic<u32>,pad0:u32,pad1:u32,pad2:u32,pairs:array<Pair>}
struct Control {error:atomic<u32>,alpha:atomic<u32>,accepted:atomic<u32>,sweep:u32,minimum:atomic<u32>,active_count:atomic<u32>,pad0:u32,pad1:u32}
struct Config {
    counts:vec4u, // moving, all vertices, faces, edges
    sizes:vec4u, // terms, nodes, contact capacity, iterations
    roots:vec4u, // face moving/fixed, edge moving/fixed
    offsets:vec4u, // primitive records, BVH order, excluded CSR, material CSR
    extra:vec4u, // material indices, unused
    physical:vec4f, // dt, damping, barrier radius, barrier stiffness
    gravity:vec4f, // w: numerical clearance
    sphere:vec4f, // centre and radius; zero radius disables
}
@group(0) @binding(0) var<storage,read_write> positions:array<vec4f>;
@group(0) @binding(1) var<storage,read_write> particles:array<Particle>;
@group(0) @binding(2) var<storage,read_write> terms:array<Term>;
@group(0) @binding(3) var<storage,read> geometry:array<u32>;
@group(0) @binding(4) var<storage,read_write> contacts:PairBuffer;
@group(0) @binding(5) var<storage,read_write> heads:array<atomic<u32>>;
@group(0) @binding(6) var<storage,read_write> nodes:array<Node>;
@group(0) @binding(7) var<storage,read_write> control:Control;
@group(0) @binding(8) var<uniform> config:Config;
fn at(v:u32,alpha:f32)->vec3f {
    if v>=config.counts.x || alpha==0 {return positions[v].xyz;}
    return positions[v].xyz+alpha*particles[v].delta.xyz;
}
fn primitive(i:u32)->vec4u {
    let p=config.offsets.x+4u*i;
    return vec4u(geometry[p],geometry[p+1u],geometry[p+2u],geometry[p+3u]);
}
fn excluded(a:u32,b:u32)->bool {
    if a==b {return true;}
    var lo=geometry[config.offsets.z+a];var hi=geometry[config.offsets.z+a+1u];
    loop {
        if lo>=hi {return false;}
        let mid=lo+(hi-lo)/2u;let v=geometry[mid];
        if v==b {return true;}if v<b {lo=mid+1u;}else{hi=mid;}
    }
    return false;
}
fn adjacent(a:vec4u,b:vec4u)->bool {
    for(var i=0u;i<a.w;i++){for(var j=0u;j<b.w;j++){if excluded(a[i],b[j]) {return true;}}}return false;
}
struct Box {lo:vec3f,hi:vec3f}
fn bounds(i:u32)->Box {
    let p=primitive(i);let a=positions[p.x].xyz;let b=positions[p.y].xyz;let c=positions[p.z].xyz;
    return Box(min(a,min(b,c)),max(a,max(b,c)));
}
fn outside(a:Box,b:Box)->bool {
    let gap=max(max(a.lo-b.hi,b.lo-a.hi)-vec3f(config.gravity.w),vec3f(0));
    // Every proposal is clamped to r/4. All unseen primitive pairs stay > r.
    let radius=1.5*config.physical.z+4*config.gravity.w;
    return dot(gap,gap)>radius*radius;
}
var<workgroup> lower:array<vec3f,64>;
var<workgroup> upper:array<vec3f,64>;
@compute @workgroup_size(64)
fn refit(@builtin(workgroup_id) group:vec3u,@builtin(local_invocation_index) lane:u32) {
    let index=group.x;if index>=config.sizes.y {return;}
    let node=nodes[index];if (node.info.w&2u)==0u {return;}var lo=vec3f(1e30);var hi=vec3f(-1e30);
    for(var j=lane;j<node.info.y;j+=64u) {
        let b=bounds(geometry[config.offsets.y+node.info.x+j]);lo=min(lo,b.lo);hi=max(hi,b.hi);
    }
    lower[lane]=lo;upper[lane]=hi;workgroupBarrier();
    for(var stride=32u;stride>0u;stride/=2u) {
        if lane<stride {lower[lane]=min(lower[lane],lower[lane+stride]);upper[lane]=max(upper[lane],upper[lane+stride]);}
        workgroupBarrier();
    }
    if lane==0u {nodes[index].lo=vec4f(lower[0],0);nodes[index].hi=vec4f(upper[0],0);}
}
@compute @workgroup_size(64)
fn search(@builtin(global_invocation_id) id:vec3u) {
    if atomicLoad(&control.error)!=0u {return;}
    let query=id.x;if query>=config.counts.y+config.counts.w {return;}
    let edge=query>=config.counts.y;
    var ai=query;var roots=config.roots.xy;
    if edge {ai=query+config.counts.z;roots=config.roots.zw;}
    let av=primitive(ai);let moving=any(av.xyz<vec3u(config.counts.x));let query_box=bounds(ai);
    for(var root_index=0u;root_index<2u;root_index++) {
        let root_index_value=roots[root_index];if root_index_value==0xffffffffu {continue;}
        let end=nodes[root_index_value].info.z;var index=root_index_value;
        loop {
            if index>=end {break;}
            let node=nodes[index];
            if (!moving && (node.info.w&2u)==0u) || outside(query_box,Box(node.lo.xyz,node.hi.xyz)) {index=node.info.z;continue;}
            if (node.info.w&1u)==0u {index++;continue;}
            index=node.info.z;
            for(var j=0u;j<node.info.y;j++) {
                let bi=geometry[config.offsets.y+node.info.x+j];if edge && ai>=bi {continue;}
                let bv=primitive(bi);if adjacent(av,bv) || outside(query_box,bounds(bi)) {continue;}
                let slot=atomicAdd(&contacts.count,1u);
                if slot>=config.sizes.z {atomicOr(&control.error,1u);continue;}
                if edge {contacts.pairs[slot].ids=vec4u(av.xy,bv.xy);contacts.pairs[slot].kind=1u;}
                else {contacts.pairs[slot].ids=vec4u(av.x,bv.xyz);contacts.pairs[slot].kind=0u;}
            }
        }
    }
}
@compute @workgroup_size(64)
fn clear_heads(@builtin(global_invocation_id) id:vec3u) {
    if id.x<config.counts.x {atomicStore(&heads[id.x],0xffffffffu);}
}
// Integer atomic insertion creates conflict-free contact incidence. Each vertex
// gathers in floating point; no fixed-point force overflow or lost float adds.
@compute @workgroup_size(64)
fn link(@builtin(global_invocation_id) id:vec3u) {
    let slot=id.x;if slot>=min(atomicLoad(&contacts.count),config.sizes.z) {return;}
    for(var j=0u;j<4u;j++) {
        let v=contacts.pairs[slot].ids[j];
        if v<config.counts.x {contacts.pairs[slot].next[j]=atomicExchange(&heads[v],slot);}
    }
}
@compute @workgroup_size(64)
fn predict(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x {return;}
    let p=positions[v];particles[v].initial=p;particles[v].delta=vec4f(0);
    let dt=config.physical.x;let damp=1+dt*config.physical.y;
    let inertia_target=p.xyz+(particles[v].velocity_mass.xyz*dt+config.gravity.xyz*dt*dt)/damp;
    particles[v].inertia_target=vec4f(inertia_target,particles[v].velocity_mass.w*damp/(dt*dt));
}
@compute @workgroup_size(64)
fn reset_stiffness(@builtin(global_invocation_id) id:vec3u) {
    if id.x<config.sizes.x {terms[id.x].c.w=select(0.1,1.0,config.sizes.w<=2u);}
}
@compute @workgroup_size(1)
fn begin_sweep() {
    atomicStore(&control.minimum,bitcast<u32>(1e30));atomicStore(&control.active_count,0u);
    atomicStore(&contacts.count,0u);atomicStore(&control.alpha,bitcast<u32>(1.0));atomicStore(&control.accepted,0u);
}
