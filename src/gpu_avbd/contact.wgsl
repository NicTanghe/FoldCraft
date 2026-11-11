struct Closest {q:vec3f,w:vec4f}
fn segment_point(p:vec3f,a:vec3f,b:vec3f)->f32 {
    let e=b-a;return clamp(dot(p-a,e)/max(dot(e,e),1e-30),0.0,1.0);
}
fn triangle_closest(p:vec3f,a:vec3f,b:vec3f,c:vec3f)->Closest {
    let e=b-a;let f=c-a;let n=cross(e,f);let nn=dot(n,n);
    var best=Closest(p-a,vec4f(1,-1,0,0));var dd=dot(best.q,best.q);
    let ab=segment_point(p,a,b);let bc=segment_point(p,b,c);let ca=segment_point(p,c,a);
    let q0=p-(a+ab*e);let q1=p-(b+bc*(c-b));let q2=p-(c+ca*(a-c));
    if dot(q0,q0)<dd {best=Closest(q0,vec4f(1,ab-1,-ab,0));dd=dot(q0,q0);}
    if dot(q1,q1)<dd {best=Closest(q1,vec4f(1,0,bc-1,-bc));dd=dot(q1,q1);}
    if dot(q2,q2)<dd {best=Closest(q2,vec4f(1,-ca,0,ca-1));dd=dot(q2,q2);}
    if nn>1e-24 {
        let u=dot(cross(p-a,f),n)/nn;let v=dot(cross(e,p-a),n)/nn;
        if u>=0 && v>=0 && u+v<=1 {
            let q=p-a-u*e-v*f;
            if dot(q,q)<dd {best=Closest(q,vec4f(1,u+v-1,-u,-v));}
        }
    } else {atomicOr(&control.error,2u);}
    return best;
}
fn edge_closest(a:vec3f,b:vec3f,c:vec3f,d:vec3f)->Closest {
    let e=b-a;let f=d-c;let ee=dot(e,e);let ff=dot(f,f);
    if min(ee,ff)<1e-24 {atomicOr(&control.error,2u);}
    var best=Closest(a-c,vec4f(1,0,-1,0));var dd=dot(best.q,best.q);
    for(var j=0u;j<4u;j++) {
        var q:vec3f;var w:vec4f;
        if j==0u {let t=segment_point(a,c,d);q=a-c-t*f;w=vec4f(1,0,t-1,-t);}
        else if j==1u {let t=segment_point(b,c,d);q=b-c-t*f;w=vec4f(0,1,t-1,-t);}
        else if j==2u {let s=segment_point(c,a,b);q=a+s*e-c;w=vec4f(1-s,s,-1,0);}
        else {let s=segment_point(d,a,b);q=a+s*e-d;w=vec4f(1-s,s,0,-1);}
        if dot(q,q)<dd {best=Closest(q,w);dd=dot(q,q);}
    }
    let n=cross(e,f);let nn=dot(n,n);
    // Cross-product formulas avoid catastrophic ee*ff-ef² cancellation.
    if nn>1e-24 {
        let s=dot(cross(c-a,f),n)/nn;let t=dot(cross(c-a,e),n)/nn;
        if s>=0 && s<=1 && t>=0 && t<=1 {
            let q=a+s*e-c-t*f;if dot(q,q)<dd {best=Closest(q,vec4f(1-s,s,t-1,-t));}
        }
    }
    return best;
}
fn closest(pair:Pair,alpha:f32)->Closest {
    let x=pair.ids;
    if pair.kind==0u {return triangle_closest(at(x.x,alpha),at(x.y,alpha),at(x.z,alpha),at(x.w,alpha));}
    return edge_closest(at(x.x,alpha),at(x.y,alpha),at(x.z,alpha),at(x.w,alpha));
}
fn scalar_barrier(d:f32,k:f32)->vec3f {
    let r=config.physical.z;
    if d>=r {return vec3f(0);}
    if !(d>config.gravity.w) {atomicOr(&control.error,4u);return vec3f(1e30,0,0);}
    let s=d*d/(r*r);let u=s-1;let l=log(s);let a=k*r*r/8;
    let first=-(2*u*l+u*u/s);let second=-(2*l+4*u/s-u*u/(s*s));
    return vec3f(-a*u*u*l,a*first*2*d/(r*r),a*(second*4*d*d/(r*r*r*r)+first*2/(r*r)));
}
fn barrier(d:f32)->vec3f {return scalar_barrier(d,config.physical.w);}
fn pair_motion(pair:Pair)->f32 {
    var a=0.0;var b=0.0;
    for(var j=0u;j<4u;j++) {
        let v=pair.ids[j];var motion=0.0;if v<config.counts.x {motion=length(particles[v].delta.xyz);}
        if (pair.kind==0u && j==0u) || (pair.kind==1u && j<2u) {a=max(a,motion);}else{b=max(b,motion);}
    }
    return a+b;
}
@compute @workgroup_size(64)
fn ccd(@builtin(global_invocation_id) id:vec3u) {
    let slot=id.x;
    if config.sphere.w>0 && slot<config.extra.y {
        let face=primitive(config.counts.y+slot);
        let q=triangle_closest(config.sphere.xyz,at(face.x,0),at(face.y,0),at(face.z,0));
        let gap=length(q.q)-config.sphere.w;
        if !(gap>config.gravity.w) {atomicOr(&control.error,4u);}
        else {
            let motion=max(length(particles[face.x].delta.xyz),max(length(particles[face.y].delta.xyz),length(particles[face.z].delta.xyz)));
            atomicMin(&control.alpha,bitcast<u32>(min(1.0,0.8*(gap-config.gravity.w)/max(motion,1e-30))));
            atomicMin(&control.minimum,bitcast<u32>(gap));if gap<config.physical.z {atomicAdd(&control.active_count,1u);}
        }
    }
    if slot>=min(atomicLoad(&contacts.count),config.sizes.z) {return;}
    let pair=contacts.pairs[slot];let d=length(closest(pair,0).q);let clearance=config.gravity.w;
    atomicMin(&control.minimum,bitcast<u32>(d));if d<config.physical.z {atomicAdd(&control.active_count,1u);}
    if !(d>clearance) {atomicOr(&control.error,4u);return;}
    let fraction=min(1.0,0.8*(d-clearance)/max(pair_motion(pair),1e-30));
    atomicMin(&control.alpha,bitcast<u32>(fraction));
}
