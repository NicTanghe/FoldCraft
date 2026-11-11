// Independent distance-slack splitting, not the published BAL formulation.
// Current/history pair regions use full primitive keys, never insertion identity.
fn contact_value(d:f32,p:Pair,k:f32,original:bool)->vec3f {
    if config.method.x==0u || original || p.rho<=0 {return scalar_barrier(d,k);}
    let q=d-p.slack+p.dual/p.rho;
    return vec3f(mathematical_barrier(p.slack,k).x+0.5*p.rho*q*q,p.rho*q,p.rho);
}
fn same_pair(a:Pair,b:Pair)->bool {return a.kind==b.kind && all(a.ids==b.ids);}
fn pivot(p:Pair)->u32 {
    for(var j=0u;j<4u;j++){if p.ids[j]<config.counts.x {return p.ids[j];}}
    return 0xffffffffu;
}
fn lookup(p:Pair,history:bool)->u32 {
    let v=pivot(p);if v==0xffffffffu {return 0xffffffffu;}
    let offset=select(0u,config.sizes.z,history);
    let head_offset=select(0u,config.counts.x,history);
    var slot=atomicLoad(&heads[head_offset+v]);var visits=0u;
    loop {
        if slot==0xffffffffu {return slot;}
        if slot>=config.sizes.z || visits>=config.sizes.z {atomicOr(&control.error,8u);return 0xffffffffu;}
        let other=contacts.pairs[offset+slot];
        if same_pair(p,other) {return slot;}
        var local=0u;for(var j=0u;j<4u;j++){if other.ids[j]==v {local=j;}}
        slot=other.next[local];visits++;
    }
    return 0xffffffffu;
}
fn mechanical(ids:vec4u,w:vec4f)->f32 {
    var compliance=0.0;
    for(var j=0u;j<4u;j++){
        let v=ids[j];
        if v<config.counts.x && positions[v].w<0.5 {
            compliance+=w[j]*w[j]/particles[v].inertia_target.w;
        }
    }
    if compliance<=1e-20 {return 0; }
    return 1.0/compliance;
}
fn choose_rho(a:f32,b:f32,k:f32)->f32 {
    if config.method.y==1u {return max(k,1e-20);}
    return max(k,clamp(sqrt(a)*sqrt(max(b,0.01*a)),0.1*a,100*a));
}
fn init_slot(slot:u32,d:f32,k:f32,a:f32,old:Pair,reuse:bool) {
    let b=scalar_barrier(d,k);
    if reuse && old.rho>0 && a>0 {
        contacts.pairs[slot].slack=old.slack;
        contacts.pairs[slot].dual=old.dual;
        contacts.pairs[slot].rho=choose_rho(a,b.z,k);
        atomicAdd(&control.matched,1u);
    } else {
        contacts.pairs[slot].slack=d;
        contacts.pairs[slot].dual=b.y;
        contacts.pairs[slot].rho=select(0.0,choose_rho(a,b.z,k),k>0 && a>0 && d<config.physical.z);
    }
}
@compute @workgroup_size(64)
fn al_initialize(@builtin(global_invocation_id) id:vec3u) {
    if atomicLoad(&control.error)!=0u {return;}
    let i=id.x;
    if i<min(atomicLoad(&contacts.count),config.sizes.z) {
        let p=contacts.pairs[i];let c=closest(p,0);
        var old=p;var found=0xffffffffu;
        if control.sweep>0u {found=lookup(p,true);}
        if found!=0xffffffffu {old=contacts.pairs[config.sizes.z+found];}
        init_slot(i,length(c.q),config.physical.w,mechanical(p.ids,c.w),old,found!=0xffffffffu);
    }
    if config.sphere.w>0 && i<config.extra.y {
        let f=primitive(config.counts.y+i);
        let c=triangle_closest(config.sphere.xyz,at(f.x,0),at(f.y,0),at(f.z,0));
        let index=2u*config.sizes.z+i;
        let k=bitcast<f32>(config.extra.z)*bitcast<f32>(geometry[config.extra.w+i]);
        init_slot(index,length(c.q)-config.sphere.w,k,mechanical(vec4u(f.xyz,0xffffffffu),vec4f(c.w.yzw,0)),contacts.pairs[index],control.sweep>0u);
    }
}
// Search incidence is frozen during this dispatch. Appended persistent pairs
// are linked only in a later clear/link pass, so lookup cannot race insertion.
@compute @workgroup_size(64)
fn al_retain(@builtin(global_invocation_id) id:vec3u) {
    let i=id.x;if i>=control.previous_count || control.sweep==0u || atomicLoad(&control.error)!=0u {return;}
    let old=contacts.pairs[config.sizes.z+i];
    if old.rho<=0 || lookup(old,false)!=0xffffffffu {return;}
    let slot=atomicAdd(&contacts.count,1u);
    if slot>=config.sizes.z {atomicOr(&control.error,1u);return;}
    contacts.pairs[slot]=old;
}
fn update_slot(slot:u32,d:f32,k:f32) {
    let p=contacts.pairs[slot];if p.rho<=0 {return;}
    var lo=max(config.physical.z*1e-12,1e-20);
    var hi=2*max(config.physical.z,d+abs(p.dual)/p.rho);
    if !(hi<1e20) {atomicOr(&control.error,32u);return;}
    if mathematical_barrier(lo,k).y-p.dual+p.rho*(lo-d)>=0 {
        atomicOr(&control.error,32u);return;
    }
    // Bisection first: predictable, monotone and independently testable.
    for(var j=0u;j<32u;j++){
        let z=0.5*(lo+hi);
        let f=mathematical_barrier(z,k).y-p.dual+p.rho*(z-d);
        if f>0 {hi=z;}else{lo=z;}
    }
    let z=0.5*(lo+hi);
    let residual=abs(mathematical_barrier(z,k).y-p.dual+p.rho*(z-d));
    let dual=p.dual+p.rho*(d-z);
    let tolerance=5e-5*max(k*config.physical.z,max(abs(p.dual),p.rho*max(d,z)))+1e-12;
    atomicMax(&control.slack_residual,bitcast<u32>(residual));
    if residual>tolerance {atomicOr(&control.error,64u);return;}
    if !(z>0 && z<1e20 && abs(dual)<1e25 && residual<1e25) {
        atomicOr(&control.error,128u);return;
    }
    atomicMax(&control.primal,bitcast<u32>(abs(d-z)));
    atomicMax(&control.slack_residual,bitcast<u32>(residual));
    atomicMax(&control.dual_change,bitcast<u32>(p.rho*abs(z-p.slack)));
    contacts.pairs[slot].slack=z;contacts.pairs[slot].dual=dual;
    // Controlled release: no silent loss of a nonzero augmented interaction.
    if d>=config.physical.z && abs(d-z)<=1e-5*config.physical.z && abs(dual)<=1e-5*k*config.physical.z {
        contacts.pairs[slot].rho=0;contacts.pairs[slot].dual=0;contacts.pairs[slot].slack=d;
    }
}
@compute @workgroup_size(64)
fn al_update(@builtin(global_invocation_id) id:vec3u) {
    if atomicLoad(&control.error)!=0u {return;}
    let i=id.x;
    if i<min(atomicLoad(&contacts.count),config.sizes.z) {
        update_slot(i,length(closest(contacts.pairs[i],0).q),config.physical.w);
    }
    if config.sphere.w>0 && i<config.extra.y {
        let f=primitive(config.counts.y+i);
        let d=length(triangle_closest(config.sphere.xyz,at(f.x,0),at(f.y,0),at(f.z,0)).q)-config.sphere.w;
        update_slot(2u*config.sizes.z+i,d,bitcast<f32>(config.extra.z)*bitcast<f32>(geometry[config.extra.w+i]));
    }
}
@compute @workgroup_size(64)
fn al_history(@builtin(global_invocation_id) id:vec3u) {
    let i=id.x;let count=min(atomicLoad(&contacts.count),config.sizes.z);
    if i<count {contacts.pairs[config.sizes.z+i]=contacts.pairs[i];}
    if i<config.counts.x {atomicStore(&heads[config.counts.x+i],atomicLoad(&heads[i]));}
    if i==0u {control.previous_count=count;}
}
@compute @workgroup_size(64)
fn al_audit(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x || positions[v].w>0.5 || atomicLoad(&control.error)!=0u {return;}
    let force=length(assemble_vertex(v,0,true).g);
    if !(force>=0 && force<1e25) {atomicOr(&control.error,16u);return;}
    atomicMax(&control.force,bitcast<u32>(force));
}
// Hardware-only scalar oracle. Not dispatched by a simulation step.
@compute @workgroup_size(64)
fn al_probe(@builtin(global_invocation_id) id:vec3u) {
    let i=id.x;if i>=config.counts.x {return;}
    let d=particles[i].initial.x;
    let before=contact_value(d,contacts.pairs[i],config.physical.w,false);
    update_slot(i,d,config.physical.w);
    particles[i].delta=vec4f(before,contacts.pairs[i].slack);
    particles[i].energy=vec4f(contacts.pairs[i].dual,contacts.pairs[i].rho,0,0);
}
