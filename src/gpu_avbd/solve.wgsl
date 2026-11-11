struct System {e:f32,g:vec3f,h:mat3x3f}
fn material_row(v:u32)->vec2u {
    return vec2u(geometry[config.offsets.w+v],geometry[config.offsets.w+v+1u]);
}
fn sphere_distance(v:u32,alpha:f32)->f32 {return length(at(v,alpha)-config.sphere.xyz)-config.sphere.w;}
fn assemble_vertex(v:u32,alpha:f32,original:bool)->System {
    let p=at(v,alpha);let q=p-particles[v].inertia_target.xyz;let inertia=particles[v].inertia_target.w;
    var system=System(0.5*inertia*dot(q,q),inertia*q,I*inertia);
    let row=material_row(v);
    for(var i=row.x;i<row.y;i++) {
        let term=terms[geometry[i]];let k=term.c.x*term.c.w;
        var c:Jet;var curvature:vec3f;
        if config.coupled.x>0u && alpha==0 {
            var local=0u;for(var j=0u;j<term.info.z;j++){if term.ids[j]==v {local=j;}}
            c=Jet(term.gradients[local].w,term.gradients[local].xyz,mat3x3f());
            curvature=vec3f(0);
        } else {
            c=factor(term,v,alpha);
            curvature=abs(k*c.v)*vec3f(length(c.h[0]),length(c.h[1]),length(c.h[2]));
        }
        system.e+=0.5*k*c.v*c.v/f32(term.info.z);
        system.g+=k*c.v*c.g;
        system.h+=k*outer(c.g,c.g)+mat3x3f(I[0]*curvature.x,I[1]*curvature.y,I[2]*curvature.z);
    }
    var slot=atomicLoad(&heads[v]);var visits=0u;
    loop {
        if slot==0xffffffffu {break;}
        if slot>=config.sizes.z || visits>=config.sizes.z {atomicOr(&control.error,8u);break;}
        let pair=contacts.pairs[slot];var local=0u;var moving=0u;
        for(var j=0u;j<4u;j++) {if pair.ids[j]==v {local=j;}if pair.ids[j]<config.counts.x {moving++;}}
        var b:vec3f;var gradient:vec3f;
        if config.coupled.x>0u && alpha==0 && !original {
            b=vec3f(pair.frozen.yz,pair.normal_curvature.w);
            gradient=pair.weights[local]*pair.normal_curvature.xyz;
        } else {
            let c=closest(pair,alpha);let d=length(c.q);b=contact_value(d,pair,config.physical.w,original);
            gradient=c.w[local]*c.q/max(d,1e-30);
        }
        system.e+=b.x/f32(moving);system.g+=b.y*gradient;system.h+=max(b.z,0)*outer(gradient,gradient);
        slot=pair.next[local];visits++;
    }
    if config.sphere.w>0 {
        let start=geometry[config.extra.x+v];let end=geometry[config.extra.x+v+1u];
        for(var i=start;i<end;i++) {
            let face=primitive(config.counts.y+geometry[i]);
            var local=0u;for(var j=0u;j<3u;j++){if face[j]==v {local=j;}}
            var b:vec3f;var gradient:vec3f;
            if config.coupled.x>0u && alpha==0 && !original {
                let pair=contacts.pairs[sphere_slot(geometry[i])];
                b=vec3f(pair.frozen.yz,pair.normal_curvature.w);
                gradient=pair.weights[local+1u]*pair.normal_curvature.xyz;
            } else {
                let closest=triangle_closest(config.sphere.xyz,at(face.x,alpha),at(face.y,alpha),at(face.z,alpha));
                let distance=length(closest.q);
                let stiffness=bitcast<f32>(config.extra.z)*bitcast<f32>(geometry[config.extra.w+geometry[i]]);
                b=scalar_barrier(distance-config.sphere.w,stiffness);
                if config.method.x==1u && !original {
                    b=contact_value(distance-config.sphere.w,contacts.pairs[2u*config.sizes.z+geometry[i]],stiffness,false);
                }
                gradient=closest.w[local+1u]*closest.q/max(distance,1e-30);
            }
            system.e+=b.x/3;system.g+=b.y*gradient;system.h+=max(b.z,0)*outer(gradient,gradient);
        }
    }
    return system;
}
@compute @workgroup_size(64)
fn propose(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x {return;}
    if atomicLoad(&control.error)!=0u {return;}
    var system:System;
    if config.coupled.x>0u {
        // cg_init already evaluated the frozen objective and gradient. Spare
        // preconditioner-column w components survive all linear iterations.
        system.g=vec3f(particles[v].cg[4].w,particles[v].cg[5].w,particles[v].cg[6].w);
    } else {
        system=assemble_vertex(v,0,false);particles[v].energy=vec4f(system.e,system.e,0,0);
    }
    var direction=vec3f(0);
    if positions[v].w<0.5 {
        if config.coupled.x>0u {direction=particles[v].delta.xyz;} else {
        let h=system.h;let g=system.g;
        let a=cross(h[1],h[2]);let b=cross(h[2],h[0]);let c=cross(h[0],h[1]);let det=dot(h[0],a);
        if !(det>0 && det<1e30) {atomicOr(&control.error,16u);}
        else {
            direction=-vec3f(dot(a,g),dot(b,g),dot(c,g))/det;
            let norm=length(direction);
            if !(norm>=0 && norm<1e20) {atomicOr(&control.error,16u);direction=vec3f(0);}
        }
        }
        let norm=length(direction);
        if !(norm>=0 && norm<1e20) {atomicOr(&control.error,16u);direction=vec3f(0);}
        direction*=min(1.0,proposal_radius()/max(norm,1e-30));
    }
    particles[v].delta=vec4f(direction,0);particles[v].energy.z=dot(system.g,direction);

}
// Evaluate the complete simultaneous trial; local descent alone does not make
// an uncoloured Jacobi update descend. All workers read the same frozen iterate.
@compute @workgroup_size(64)
fn trial(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x {return;}
    if atomicLoad(&control.accepted)!=0u || atomicLoad(&control.error)!=0u {return;}
    particles[v].energy.y=energy_vertex(v,bitcast<f32>(atomicLoad(&control.alpha)));
}
var<workgroup> sums:array<vec3f,64>;
var<workgroup> acceptance_done:u32;
@compute @workgroup_size(64)
fn accept(@builtin(local_invocation_index) lane:u32) {
    // Broadcast before branching, so every lane reaches cooperative barriers.
    if config.method.z==0u {
        if lane==0u {acceptance_done=atomicLoad(&control.accepted)|atomicLoad(&control.error);}
        let done=workgroupUniformLoad(&acceptance_done);
        if done!=0u {return;}
    }
    var sum=vec3f(0);
    for(var v=lane;v<config.counts.x;v+=64u) {sum+=particles[v].energy.xyz;}
    sums[lane]=sum;workgroupBarrier();
    for(var stride=32u;stride>0u;stride/=2u) {
        if lane<stride {sums[lane]+=sums[lane+stride];}workgroupBarrier();
    }
    if lane==0u && atomicLoad(&control.accepted)==0u && atomicLoad(&control.error)==0u {
        atomicAdd(&control.trials,1u);
        let total=sums[0];let alpha=bitcast<f32>(atomicLoad(&control.alpha));
        // Strict global Armijo; failed/exhausted trials leave the iterate intact.
        if total.y>=0 && total.y<1e25 && total.y<=total.x+1e-4*alpha*total.z {
            atomicStore(&control.accepted,1u);
        }else {atomicStore(&control.alpha,bitcast<u32>(0.5*alpha));}
    }
}
@compute @workgroup_size(64)
fn commit(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x {return;}
    if atomicLoad(&control.accepted)==0u || atomicLoad(&control.error)!=0u {return;}
    positions[v]=vec4f(at(v,bitcast<f32>(atomicLoad(&control.alpha))),positions[v].w);
}
@compute @workgroup_size(64)
fn ramp(@builtin(global_invocation_id) id:vec3u) {
    let i=id.x;if i>=config.sizes.x || atomicLoad(&control.error)!=0u {return;}
    let c=factor_value(terms[i],0);
    let schedule=min(1.0,f32(control.sweep+2u)/max(1.0,f32(config.sizes.w/2u)));
    terms[i].c.w=min(1.0,max(schedule,terms[i].c.w+10*abs(c)));
}
@compute @workgroup_size(1)
fn end_sweep() {control.sweep++;}
@compute @workgroup_size(64)
fn finish(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x || atomicLoad(&control.error)!=0u {return;}
    var velocity=(positions[v].xyz-particles[v].initial.xyz)/config.physical.x;
    if positions[v].w>0.5 {velocity=vec3f(0);}
    particles[v].velocity_mass=vec4f(velocity,particles[v].velocity_mass.w);
}
// Opt-in numerical oracle, used only by the hardware validation test.
@compute @workgroup_size(64)
fn probe_material(@builtin(global_invocation_id) id:vec3u) {
    let v=id.x;if v>=config.counts.x {return;}
    let s=assemble_vertex(v,0,false);particles[v].delta=vec4f(s.g,energy_vertex(v,0));
}

// For <=64 vertices, all trial energies and the global reduction fit in one
// workgroup. This preserves the same Armijo test and reduction order while
// avoiding 32 host-recorded dispatches per sweep. Larger meshes use the
// multi-workgroup trial/reduction path.
var<workgroup> trial_alpha:u32;
@compute @workgroup_size(64)
fn small_line_search(@builtin(local_invocation_index) lane:u32) {
    for(var attempt=0u;attempt<16u;attempt++) {
        if lane==0u {
            acceptance_done=atomicLoad(&control.accepted)|atomicLoad(&control.error);
            trial_alpha=atomicLoad(&control.alpha);
        }
        let done=workgroupUniformLoad(&acceptance_done);
        let alpha=bitcast<f32>(workgroupUniformLoad(&trial_alpha));
        if done!=0u {break;}
        var sum=vec3f(0);
        if lane<config.counts.x {
            sum=vec3f(particles[lane].energy.x,energy_vertex(lane,alpha),particles[lane].energy.z);
        }
        sums[lane]=sum;workgroupBarrier();
        for(var stride=32u;stride>0u;stride/=2u) {
            if lane<stride {sums[lane]+=sums[lane+stride];}workgroupBarrier();
        }
        if lane==0u && atomicLoad(&control.error)==0u {
            atomicAdd(&control.trials,1u);
            let total=sums[0];
            if total.y>=0 && total.y<1e25 && total.y<=total.x+1e-4*alpha*total.z {
                atomicStore(&control.accepted,1u);
            } else {atomicStore(&control.alpha,bitcast<u32>(0.5*alpha));}
        }
        workgroupBarrier();
    }
}

fn energy_vertex(v:u32,alpha:f32)->f32 {
    let q=at(v,alpha)-particles[v].inertia_target.xyz;
    var energy=0.5*particles[v].inertia_target.w*dot(q,q);let row=material_row(v);
    for(var i=row.x;i<row.y;i++) {
        let t=terms[geometry[i]];let c=factor_value(t,alpha);
        energy+=0.5*t.c.x*t.c.w*c*c/f32(t.info.z);
    }
    var slot=atomicLoad(&heads[v]);var visits=0u;
    loop {
        if slot==0xffffffffu {break;}
        if slot>=config.sizes.z || visits>=config.sizes.z {atomicOr(&control.error,8u);break;}
        let pair=contacts.pairs[slot];var local=0u;var moving=0u;
        for(var j=0u;j<4u;j++){if pair.ids[j]==v {local=j;}if pair.ids[j]<config.counts.x {moving++;}}
        energy+=contact_value(length(closest(pair,alpha).q),pair,config.physical.w,false).x/f32(moving);
        slot=pair.next[local];visits++;
    }
    if config.sphere.w>0 {
        let start=geometry[config.extra.x+v];let end=geometry[config.extra.x+v+1u];
        for(var i=start;i<end;i++) {
            let f=primitive(config.counts.y+geometry[i]);
            let distance=length(triangle_closest(config.sphere.xyz,at(f.x,alpha),at(f.y,alpha),at(f.z,alpha)).q)-config.sphere.w;
            let k=bitcast<f32>(config.extra.z)*bitcast<f32>(geometry[config.extra.w+geometry[i]]);
            var b=scalar_barrier(distance,k);
            if config.method.x==1u {b=contact_value(distance,contacts.pairs[2u*config.sizes.z+geometry[i]],k,false);}
            energy+=b.x/3;
        }
    }
    return energy;
}
