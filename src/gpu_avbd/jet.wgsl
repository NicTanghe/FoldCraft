// Second-order local AD for scalar material factors, at one vertex (3 DOFs).
struct Jet { v:f32, g:vec3f, h:mat3x3f }
const I=mat3x3f(vec3f(1,0,0),vec3f(0,1,0),vec3f(0,0,1));
fn outer(a:vec3f,b:vec3f)->mat3x3f {return mat3x3f(a*b.x,a*b.y,a*b.z);}
fn jc(v:f32)->Jet {return Jet(v,vec3f(0),mat3x3f());}
fn ja(a:Jet,b:Jet)->Jet {return Jet(a.v+b.v,a.g+b.g,a.h+b.h);}
fn js(a:Jet,b:Jet)->Jet {return Jet(a.v-b.v,a.g-b.g,a.h-b.h);}
fn scale(a:Jet,b:f32)->Jet {return Jet(a.v*b,a.g*b,a.h*b);}
fn jm(a:Jet,b:Jet)->Jet {return Jet(a.v*b.v,a.g*b.v+b.g*a.v,a.h*b.v+b.h*a.v+outer(a.g,b.g)+outer(b.g,a.g));}
fn unary(a:Jet,v:f32,g:f32,h:f32)->Jet {return Jet(v,a.g*g,a.h*g+outer(a.g,a.g)*h);}
fn root(a:Jet)->Jet {let r=sqrt(max(a.v,1e-30));return unary(a,r,0.5/r,-0.25/(r*r*r));}
fn inv(a:Jet)->Jet {let r=1.0/a.v;return unary(a,r,-r*r,2*r*r*r);}
struct JV {x:Jet,y:Jet,z:Jet}
fn constant(p:vec3f)->JV {return JV(jc(p.x),jc(p.y),jc(p.z));}
fn variable(p:vec3f)->JV {return JV(Jet(p.x,I[0],mat3x3f()),Jet(p.y,I[1],mat3x3f()),Jet(p.z,I[2],mat3x3f()));}
fn va(a:JV,b:JV)->JV {return JV(ja(a.x,b.x),ja(a.y,b.y),ja(a.z,b.z));}
fn vs(a:JV,b:JV)->JV {return JV(js(a.x,b.x),js(a.y,b.y),js(a.z,b.z));}
fn vm(a:JV,b:f32)->JV {return JV(scale(a.x,b),scale(a.y,b),scale(a.z,b));}
fn vj(a:JV,b:Jet)->JV {return JV(jm(a.x,b),jm(a.y,b),jm(a.z,b));}
fn jd(a:JV,b:JV)->Jet {return ja(ja(jm(a.x,b.x),jm(a.y,b.y)),jm(a.z,b.z));}
fn jcross(a:JV,b:JV)->JV {return JV(js(jm(a.y,b.z),jm(a.z,b.y)),js(jm(a.z,b.x),jm(a.x,b.z)),js(jm(a.x,b.y),jm(a.y,b.x)));}
fn normal(a:JV)->JV {return vj(a,inv(root(jd(a,a))));}
// U = k/2 C². Finite AVBD uses no hard-constraint multiplier for these factors.
fn factor(t:Term,v:u32,alpha:f32)->Jet {
    var x:array<JV,4>;
    for(var j=0u;j<4u;j++) {
        let p=at(t.ids[j],alpha);
        x[j]=constant(p);
        if t.ids[j]==v {x[j]=variable(p);}
    }
    if t.info.x==0u {
        let e=vs(x[1],x[0]);let f=vs(x[2],x[0]);
        let warp=va(vm(e,t.a.y),vm(f,t.a.z));let weft=va(vm(e,t.b.y),vm(f,t.b.z));
        if t.info.y==4u {return jd(warp,weft);}
        var q=warp;if t.info.y>=2u {q=weft;}
        let strain=scale(js(jd(q,q),jc(1)),0.5);
        if t.info.y%2u==0u {return strain;}
        return scale(js(ja(strain,root(ja(jm(strain,strain),jc(t.c.y*t.c.y)))),jc(t.c.y)),0.5);
    }
    if t.info.x==1u {
        let e=vs(x[1],x[0]);let a=normal(jcross(e,vs(x[2],x[0])));let b=normal(jcross(vs(x[3],x[0]),e));
        let d=vs(a,b);if t.info.y==0u{return d.x;}if t.info.y==1u{return d.y;}return d.z;
    }
    var first=x[0].x;if t.info.y==1u {first=x[0].y;}if t.info.y==2u {first=x[0].z;}
    var value=js(scale(first,t.b.w),jc(t.c.z));
    for(var j=1u;j<t.info.z;j++) {
        var coordinate=x[j].x;if t.info.y==1u {coordinate=x[j].y;}if t.info.y==2u {coordinate=x[j].z;}
        value=ja(value,scale(js(coordinate,first),t.a[j]));
    }
    return value;
}
