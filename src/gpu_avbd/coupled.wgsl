// Inexact PCG of the coupled positive AVBD/Gauss-Newton curvature.
// No host iteration decisions, floating atomics, or changes to physical energy.
@compute @workgroup_size(64)
fn cache_material(@builtin(global_invocation_id) id:vec3u) {
    let i=id.x;if i>=config.sizes.x || atomicLoad(&control.error)!=0u {return;}
    let t=terms[i];
    for(var j=0u;j<t.info.z;j++){
        let c=factor(t,t.ids[j],0);
        terms[i].gradients[j]=vec4f(c.g,c.v);
        // Coupled Gauss-Newton uses J^T k J. Adding per-vertex row-norm
        // curvature would resist rigid translation of a stressed patch.
        terms[i].curvature[j]=vec4f(0);
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
    particles[v].cg[4]=vec4f(inverse[0],0);particles[v].cg[5]=vec4f(inverse[1],0);particles[v].cg[6]=vec4f(inverse[2],0);
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
        value+=t.c.x*t.c.w*t.gradients[local].xyz*dotp+t.curvature[local].xyz*p;
    }
    var slot=atomicLoad(&heads[v]);var visits=0u;
    loop {
        if slot==0xffffffffu {break;}
        if slot>=config.sizes.z || visits>=config.sizes.z {atomicOr(&control.error,8u);return;}
        let pair=contacts.pairs[slot];let c=closest(pair,0);let d=length(c.q);
        let b=contact_value(d,pair,config.physical.w,false);let normal=c.q/max(d,1e-30);
        var local=0u;var delta=vec3f(0);
        for(var j=0u;j<4u;j++){if pair.ids[j]==v {local=j;}delta+=c.w[j]*search_vector(pair.ids[j]);}
        value+=max(b.z,0)*c.w[local]*normal*dot(normal,delta);
        slot=pair.next[local];visits++;
    }
    if config.sphere.w>0 {
        let start=geometry[config.extra.x+v];let end=geometry[config.extra.x+v+1u];
        for(var i=start;i<end;i++){
            let f=primitive(config.counts.y+geometry[i]);
            let c=triangle_closest(config.sphere.xyz,at(f.x,0),at(f.y,0),at(f.z,0));let distance=length(c.q);
            let k=bitcast<f32>(config.extra.z)*bitcast<f32>(geometry[config.extra.w+geometry[i]]);
            var b=scalar_barrier(distance-config.sphere.w,k);
            if config.method.x==1u {b=contact_value(distance-config.sphere.w,contacts.pairs[2u*config.sizes.z+geometry[i]],k,false);}
            var local=0u;var delta=vec3f(0);
            for(var j=0u;j<3u;j++){if f[j]==v {local=j;}delta+=c.w[j+1u]*search_vector(f[j]);}
            let normal=c.q/max(distance,1e-30);
            value+=max(b.z,0)*c.w[local+1u]*normal*dot(normal,delta);
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
