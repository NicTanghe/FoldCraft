// Inexact PCG of the coupled positive AVBD/Gauss-Newton curvature.
// No host iteration decisions, floating atomics, or changes to physical energy.
@compute @workgroup_size(64)
fn cache_material(@builtin(global_invocation_id) id:vec3u) {
    let i=id.x;if i>=config.sizes.x || atomicLoad(&control.error)!=0u {return;}
    let t=terms[i];
    // Coupled Gauss-Newton uses J^T k J. Per-vertex row-norm curvature
    // would resist rigid translation of a stressed patch.
    for(var j=0u;j<t.info.z;j++){
        let c=factor(t,t.ids[j],0);
        terms[i].gradients[j]=vec4f(c.g,c.v);
    }
}
fn precondition(v:u32,r:vec3f)->vec3f {
    return mat3x3f(particles[v].cg[4].xyz,particles[v].cg[5].xyz,particles[v].cg[6].xyz)*r;
}
@compute @workgroup_size(64)
fn cg_init(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x || atomicLoad(&control.error)!=0u {return;}
    let s=assemble_vertex(v,0,false);
    var r=-s.g;var inverse=mat3x3f();
    if positions[v].w>0.5 {r=vec3f(0);} else {
        // Regularize only the preconditioner. J^T k J can be almost singular
        // in f32 even though the full operator retains positive inertia.
        let floor=max(1e-5*(s.h[0].x+s.h[1].y+s.h[2].z),1e-12);
        let h=s.h+I*floor;let a=cross(h[1],h[2]);let b=cross(h[2],h[0]);let c=cross(h[0],h[1]);let det=dot(h[0],a);
        if !(det>0 && det<1e30) {atomicOr(&control.error,256u);return;}
        inverse=transpose(mat3x3f(a,b,c))*(1.0/det);
    }
    particles[v].cg[4]=vec4f(inverse[0],s.g.x);particles[v].cg[5]=vec4f(inverse[1],s.g.y);particles[v].cg[6]=vec4f(inverse[2],s.g.z);
    particles[v].energy=vec4f(s.e,s.e,0,0);
    let z=inverse*r;
    particles[v].cg[0]=vec4f(r,0);particles[v].cg[1]=vec4f(z,0);particles[v].cg[2]=vec4f(z,0);
    particles[v].delta=vec4f(0);
}
fn search_vector(v:u32)->vec3f {
    if v>=config.counts.x {return vec3f(0);}
    return particles[v].cg[2].xyz;
}
@compute @workgroup_size(64)
fn cg_matvec(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x || atomicLoad(&control.error)!=0u {return;}
    let p=search_vector(v);var value=particles[v].inertia_target.w*p;
    let row=material_row(v);
    for(var i=row.x;i<row.y;i++){
        let t=terms[geometry[i]];var local=0u;var dotp=0.0;
        for(var j=0u;j<t.info.z;j++){
            if t.ids[j]==v {local=j;}
            dotp+=dot(t.gradients[j].xyz,search_vector(t.ids[j]));
        }
        value+=t.c.x*t.c.w*t.gradients[local].xyz*dotp;
    }
    var slot=atomicLoad(&heads[v]);var visits=0u;
    loop {
        if slot==0xffffffffu {break;}
        if slot>=config.sizes.z || visits>=config.sizes.z {atomicOr(&control.error,8u);return;}
        let pair=contacts.pairs[slot];
        let normal=pair.normal_curvature.xyz;
        var local=0u;var delta=vec3f(0);
        for(var j=0u;j<4u;j++){if pair.ids[j]==v {local=j;}delta+=pair.weights[j]*search_vector(pair.ids[j]);}
        value+=pair.normal_curvature.w*pair.weights[local]*normal*dot(normal,delta);
        slot=pair.next[local];visits++;
    }
    if config.sphere.w>0 {
        let start=geometry[config.extra.x+v];let end=geometry[config.extra.x+v+1u];
        for(var i=start;i<end;i++){
            let f=primitive(config.counts.y+geometry[i]);
            let pair=contacts.pairs[sphere_slot(geometry[i])];
            var local=0u;var delta=vec3f(0);
            for(var j=0u;j<3u;j++){if f[j]==v {local=j;}delta+=pair.weights[j+1u]*search_vector(f[j]);}
            let normal=pair.normal_curvature.xyz;
            value+=pair.normal_curvature.w*pair.weights[local+1u]*normal*dot(normal,delta);
        }
    }
    if positions[v].w>0.5 {value=vec3f(0);}
    particles[v].cg[3]=vec4f(value,0);
}
var<workgroup> cg_sums:array<vec2f,64>;
@compute @workgroup_size(64)
fn cg_alpha(@builtin(local_invocation_index) lane:u32) {
    var sum=vec2f(0);
    for(var v=lane;v<config.counts.x;v+=64u){
        sum+=vec2f(dot(particles[v].cg[0].xyz,particles[v].cg[1].xyz),dot(particles[v].cg[2].xyz,particles[v].cg[3].xyz));
    }
    cg_sums[lane]=sum;workgroupBarrier();
    for(var stride=32u;stride>0u;stride/=2u){if lane<stride {cg_sums[lane]+=cg_sums[lane+stride];}workgroupBarrier();}
    if lane==0u {
        let s=cg_sums[0];var alpha=0.0;
        if s.x>1e-25 && s.y>1e-30 {alpha=s.x/s.y;}
        if !(alpha>=0 && alpha<1e20) {atomicOr(&control.error,512u);alpha=0;}
        control.pad0=bitcast<u32>(s.x);control.pad1=bitcast<u32>(alpha);
    }
}
@compute @workgroup_size(64)
fn cg_update(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x || atomicLoad(&control.error)!=0u {return;}
    let alpha=bitcast<f32>(control.pad1);
    particles[v].delta+=alpha*particles[v].cg[2];
    let r=particles[v].cg[0].xyz-alpha*particles[v].cg[3].xyz;
    let z=precondition(v,r);
    if !all(abs(z)<vec3f(1e25)) {atomicOr(&control.error,1024u);return;}
    particles[v].cg[0]=vec4f(r,0);particles[v].cg[1]=vec4f(z,0);
}
@compute @workgroup_size(64)
fn cg_beta(@builtin(local_invocation_index) lane:u32) {
    var sum=0.0;
    for(var v=lane;v<config.counts.x;v+=64u){sum+=dot(particles[v].cg[0].xyz,particles[v].cg[1].xyz);}
    cg_sums[lane]=vec2f(sum,0);workgroupBarrier();
    for(var stride=32u;stride>0u;stride/=2u){if lane<stride {cg_sums[lane]+=cg_sums[lane+stride];}workgroupBarrier();}
    if lane==0u {
        let old=bitcast<f32>(control.pad0);var beta=0.0;
        if old>1e-25 {beta=max(cg_sums[0].x,0)/old;}
        control.pad2=bitcast<u32>(beta);
    }
}
@compute @workgroup_size(64)
fn cg_search(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x || atomicLoad(&control.error)!=0u {return;}
    particles[v].cg[2]=particles[v].cg[1]+bitcast<f32>(control.pad2)*particles[v].cg[2];
}

fn sphere_slot(face:u32)->u32 {return (1u+config.method.x)*config.sizes.z+face;}
// The iterate and AL variables stay frozen throughout PCG. Recompute once per
// nonlinear sweep; line search/CCD still evaluate their actual trial geometry.
@compute @workgroup_size(64)
fn cache_contacts(@builtin(global_invocation_id) id:vec3u) {
    let i=id.x;if atomicLoad(&control.error)!=0u {return;}
    if i<min(atomicLoad(&contacts.count),config.sizes.z) {
        let pair=contacts.pairs[i];let c=closest(pair,0);let d=length(c.q);
        let b=contact_value(d,pair,config.physical.w,false);
        contacts.pairs[i].normal_curvature=vec4f(c.q/max(d,1e-30),max(b.z,0));
        contacts.pairs[i].weights=c.w;
        contacts.pairs[i].frozen=vec4f(d,b.x,b.y,0);
    }
    if config.sphere.w>0 && i<config.extra.y {
        let f=primitive(config.counts.y+i);
        let c=triangle_closest(config.sphere.xyz,at(f.x,0),at(f.y,0),at(f.z,0));let d=length(c.q);
        let k=bitcast<f32>(config.extra.z)*bitcast<f32>(geometry[config.extra.w+i]);
        let slot=sphere_slot(i);var b=scalar_barrier(d-config.sphere.w,k);
        if config.method.x==1u {b=contact_value(d-config.sphere.w,contacts.pairs[slot],k,false);}
        contacts.pairs[slot].normal_curvature=vec4f(c.q/max(d,1e-30),max(b.z,0));
        contacts.pairs[slot].weights=c.w;
        contacts.pairs[slot].frozen=vec4f(d-config.sphere.w,b.x,b.y,0);
    }
}


// Temporary active-curvature incidence, only for frozen PCG products. The
// complete incidence is restored before local bounds, CCD and trial energy.
@compute @workgroup_size(64)
fn link_coupled(@builtin(global_invocation_id) id:vec3u) {
    let slot=id.x;
    if slot>=min(atomicLoad(&contacts.count),config.sizes.z) {return;}
    if !(contacts.pairs[slot].normal_curvature.w>0) {return;}
    for(var j=0u;j<4u;j++) {
        let v=contacts.pairs[slot].ids[j];
        if v<config.counts.x {contacts.pairs[slot].next[j]=atomicExchange(&heads[v],slot);}
    }
}
