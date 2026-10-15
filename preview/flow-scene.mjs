// Closed browser-native scene. No imported geometry, textures, or shader input.
import { FLOW_NODES, FLOW_EDGES } from './flow-simulation.mjs';

const rad = value => value * Math.PI / 180;
const clamp = (value, min, max, fallback) => Number.isFinite(value) ? Math.max(min, Math.min(max, value)) : fallback;
export const defaultCamera = Object.freeze({ yaw: -12, pitch: 44, zoom: .95 });
export function normalizeCamera(value = {}) { value = value && typeof value === 'object' ? value : {}; return { yaw: clamp(value.yaw, -24, 24, -12), pitch: clamp(value.pitch, 36, 52, 44), zoom: clamp(value.zoom, .85, 1.15, .95) }; }
const sub = (a, b) => a.map((v, i) => v - b[i]);
const cross = (a, b) => [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]];
const dot = (a, b) => a.reduce((n, v, i) => n + v*b[i], 0);
const unit = a => { const length = Math.hypot(...a); return length ? a.map(v => v/length) : [0, 1, 0]; };
const add = (a, b) => a.map((v, i) => v+b[i]);
const scale = (a, amount) => a.map(v => v*amount);
export const worldPoint = (x, y) => [(x - 470)/115, .15, (y - 265)/72];
export const modelTops=Object.freeze({agent:.41,gateway:.76,policy:.455,human:.75,authority:.255,executor:.56,tool:.307,evidence:.62});
export const SCENE_POSES = Object.freeze(FLOW_NODES.map(node => Object.freeze({ id: node.id, position: Object.freeze(worldPoint(node.x+95, node.y+50)) })));
export function multiplyMatrix(a, b) {
  const result = new Array(16).fill(0);
  for (let col=0; col<4; col++) for (let row=0; row<4; row++) for (let k=0; k<4; k++) result[col*4+row] += a[k*4+row]*b[col*4+k];
  return result;
}
function normalizeSectionFrame(value) {
  if (!value || !Array.isArray(value.center) || value.center.length !== 3 || !value.center.every(v => Number.isFinite(v) && Math.abs(v) <= 12) || !Number.isFinite(value.framing) || value.framing < .65 || value.framing > 1.6 || !Array.isArray(value.nodeIds) || !value.nodeIds.length || value.nodeIds.length > 24 || new Set(value.nodeIds).size !== value.nodeIds.length || !value.nodeIds.every(id => typeof id === 'string' && /^[a-zA-Z0-9_-]{1,128}$/.test(id))) return null;
  return { center: [...value.center], framing: value.framing, nodeIds: [...value.nodeIds] };
}
export function cameraMatrix(camera, aspect, framing = 1, frame = null) {
  const section = normalizeSectionFrame(frame), center = section?.center ?? [0,0,0];
  const c = normalizeCamera(camera), yaw = rad(c.yaw), pitch = rad(c.pitch), distance = 11.4/c.zoom/clamp(framing,.4,1,1)/(section?.framing ?? 1);
  const direction = [Math.sin(yaw)*Math.cos(pitch)*distance, Math.sin(pitch)*distance, Math.cos(yaw)*Math.cos(pitch)*distance];
  const eye = add(center, direction);
  const z = unit(direction), x = unit(cross([0,1,0], z)), y = cross(z, x);
  const view = [x[0],y[0],z[0],0,x[1],y[1],z[1],0,x[2],y[2],z[2],0,-dot(x,eye),-dot(y,eye),-dot(z,eye),1];
  const f = 1/Math.tan(rad(33)/2), a = clamp(aspect,.5,4,1.5), near=.1, far=60;
  const projection = [f/a,0,0,0,0,f,0,0,0,0,(far+near)/(near-far),-1,0,0,2*far*near/(near-far),0];
  return { matrix: multiplyMatrix(projection, view), eye };
}
export function projectPoint(point, matrix) {
  if (!Array.isArray(point) || point.length !== 3 || !point.every(Number.isFinite) || !Array.isArray(matrix) || matrix.length !== 16 || !matrix.every(Number.isFinite)) return null;
  const vector = [...point,1], clip = [0,0,0,0];
  for (let row=0; row<4; row++) for (let k=0; k<4; k++) clip[row] += matrix[k*4+row]*vector[k];
  if (clip[3] <= 0) return null;
  const ndc = clip.slice(0,3).map(v => v/clip[3]);
  return { x: (ndc[0]+1)*50, y:(1-ndc[1])*50, visible: ndc.every(v=>v>=-1&&v<=1) };
}

export function sceneRoute(id) {
  const edge=FLOW_EDGES.find(value=>value.id===id);if(!edge)return null;
  const from=SCENE_POSES.find(value=>value.id===edge.from).position,to=SCENE_POSES.find(value=>value.id===edge.to).position;
  let points=[from,to];
  if(['denial','refusal','gap'].includes(id)){const z=id==='denial'?-2.7:id==='refusal'?2.55:2.95;points=[from,[from[0],.2,z],[to[0],.2,z],to];}
  points=points.map(p=>[p[0],.2,p[2]]);points[0]=add(points[0],scale(unit(sub(points[1],points[0])),.56));points[points.length-1]=sub(points.at(-1),scale(unit(sub(points.at(-1),points.at(-2))),.56));
  return points;
}
export function pointOnSceneEdge(id,progress) {
  const points=sceneRoute(id);if(!points||!Number.isFinite(progress)||progress<0||progress>1)return null;if(progress===0)return points[0];if(progress===1)return points.at(-1);
  const lengths=points.slice(1).map((p,i)=>Math.hypot(...sub(p,points[i])));let left=progress*lengths.reduce((n,v)=>n+v,0);
  for(let i=0;i<lengths.length;i++){if(left<=lengths[i]||i===lengths.length-1)return add(points[i],scale(sub(points[i+1],points[i]),left/lengths[i]));left-=lengths[i];}return null;
}

export function buildSceneGeometry({poses=SCENE_POSES,edges=FLOW_EDGES,route=sceneRoute}={}) {
  const values=[], groups=[];
  const triangle = (a,b,c,normal) => { const n=normal??unit(cross(sub(b,a),sub(c,a))); for(const p of [a,b,c]) values.push(...p,...n); };
  const vertexTriangle=(points,normals)=>{for(let i=0;i<3;i++)values.push(...points[i],...normals[i]);};
  const roundedBox=(p,size,radius=.05)=>{
    const h=size.map(v=>v/2),r=Math.min(radius,...h)*.9;
    const axis=h.map(v=>[-v,-v+r*.5,-v+r,-(v-r)/3,(v-r)/3,v-r,v-r*.5,v]);
    const faces=[[0,-1],[0,1],[1,-1],[1,1],[2,-1],[2,1]];
    for(const [fixed,sign]of faces){const other=[0,1,2].filter(v=>v!==fixed);
      const sample=(i,j)=>{const q=[0,0,0];q[fixed]=h[fixed]*sign;q[other[0]]=axis[other[0]][i];q[other[1]]=axis[other[1]][j];const core=q.map((v,k)=>Math.max(-h[k]+r,Math.min(h[k]-r,v))),n=unit(sub(q,core));return {p:add(p,add(core,scale(n,r))),n};};
      for(let i=0;i<7;i++)for(let j=0;j<7;j++){const a=sample(i,j),b=sample(i+1,j),c=sample(i+1,j+1),d=sample(i,j+1);vertexTriangle([a.p,b.p,c.p],[a.n,b.n,c.n]);vertexTriangle([a.p,c.p,d.p],[a.n,c.n,d.n]);}
    }
  };
  const prism=(p,outline,height)=>{const bottom=outline.map(v=>add(p,[v[0],-height/2,v[1]])),top=outline.map(v=>add(p,[v[0],height/2,v[1]]));for(let i=0;i<outline.length;i++){const j=(i+1)%outline.length;triangle(bottom[i],bottom[j],top[j]);triangle(bottom[i],top[j],top[i]);triangle(p.map((v,k)=>k===1?v+height/2:v),top[i],top[j],[0,1,0]);triangle(p.map((v,k)=>k===1?v-height/2:v),bottom[j],bottom[i],[0,-1,0]);}};
  const cylinder = (a,b,radius,sides=24) => {
    const axis=unit(sub(b,a)), u=unit(cross(axis,Math.abs(axis[1])>.9?[1,0,0]:[0,1,0])), v=cross(axis,u);
    for(let i=0;i<sides;i++) {
      const n1=add(scale(u,Math.cos(i*2*Math.PI/sides)),scale(v,Math.sin(i*2*Math.PI/sides)));
      const n2=add(scale(u,Math.cos((i+1)*2*Math.PI/sides)),scale(v,Math.sin((i+1)*2*Math.PI/sides)));
      const p1=add(a,scale(n1,radius)),p2=add(a,scale(n2,radius)),q1=add(b,scale(n1,radius)),q2=add(b,scale(n2,radius));
      vertexTriangle([p1,q1,q2],[n1,n1,n2]);vertexTriangle([p1,q2,p2],[n1,n2,n2]);triangle(a,p2,p1,scale(axis,-1));triangle(b,q1,q2,axis);
    }
  };
  const sphere = (p,radius,stretch=[1,1,1]) => {
    const point=(i,j)=>add(p,[stretch[0]*radius*Math.sin(i*Math.PI/12)*Math.cos(j*Math.PI/12),stretch[1]*radius*Math.cos(i*Math.PI/12),stretch[2]*radius*Math.sin(i*Math.PI/12)*Math.sin(j*Math.PI/12)]);
    for(let i=0;i<12;i++) for(let j=0;j<24;j++) { const a=point(i,j),b=point(i+1,j),c=point(i+1,j+1),d=point(i,j+1); triangle(a,b,c,unit(sub(add(add(a,b),c),scale(p,3))));triangle(a,c,d,unit(sub(add(add(a,c),d),scale(p,3)))); }
  };
  const ring=(p,r,tube,vertical=false)=>{const point=(i,j)=>{const q=[(r+tube*Math.cos(j*Math.PI/4))*Math.cos(i*Math.PI/16),tube*Math.sin(j*Math.PI/4),(r+tube*Math.cos(j*Math.PI/4))*Math.sin(i*Math.PI/16)];return add(p,vertical?[q[0],q[2],q[1]]:q);};for(let i=0;i<32;i++)for(let j=0;j<8;j++){const a=point(i,j),b=point(i+1,j),c=point(i+1,j+1),d=point(i,j+1);triangle(a,b,c);triangle(a,c,d);}};
  const group=(id,color,fn)=>{const start=values.length/6;fn();groups.push({id,color,start,count:values.length/6-start});};
  group('grid',[.15,.23,.28],()=>{for(let i=-5;i<=5;i++){cylinder([i,-.04,-3.4],[i,-.04,3.4],.006,4);cylinder([-5,-.04,i*.65],[5,-.04,i*.65],.006,4);}});
  for(const edge of edges) group('edge:'+edge.id, edge.kind==='control'||edge.id==='authorize'?[1,.47,.16]:edge.kind==='data'?[.18,.65,.71]:['evaluate','recommend'].includes(edge.id)?[.57,.42,.95]:['outcome','refusal','gap','denial'].includes(edge.id)?[.18,.65,.71]:[.36,.48,.55],()=>{
    const points=route(edge.id);
    for(let i=1;i<points.length;i++) {
      if(edge.kind==='data') {
        const delta=sub(points[i],points[i-1]),length=Math.hypot(...delta),count=Math.ceil(length/.16);
        for(let j=0;j<count;j++)cylinder(add(points[i-1],scale(delta,j/count)),add(points[i-1],scale(delta,Math.min(1,(j+.6)/count))),.016,6);
      } else cylinder(points[i-1],points[i],.022,6);
    }
    const end=points.at(-1),direction=unit(sub(end,points.at(-2))),side=scale(unit(cross(direction,[0,1,0])),.08),base=sub(end,scale(direction,.16));
    triangle(add(base,side),end,sub(base,side),[0,1,0]);
  });
  const colors={agent:[.24,.57,.66],gateway:[.17,.53,.49],policy:[.52,.4,.78],human:[.73,.52,.35],authority:[.91,.39,.17],executor:[.62,.29,.27],tool:[.33,.47,.68],evidence:[.23,.58,.53]};
  for(const node of poses){
    const role=node.kind??node.id;
    const [x,,z]=node.position;
    group('base:'+node.id,[.17,.22,.26],()=>cylinder([x,.02,z],[x,.07,z],.54,32));
    group('rim:'+node.id,colors[role],()=>ring([x,.075,z],.51,.012));
    group('node:'+node.id,colors[role],()=>{
      if(role==='agent')prism([x,.28,z],[[-.39,-.22],[.24,-.22],[.42,0],[.24,.22],[-.39,.22]],.26);
      else if(role==='gateway'){roundedBox([x-.32,.37,z],[.14,.58,.18]);roundedBox([x+.32,.37,z],[.14,.58,.18]);roundedBox([x,.7,z],[.8,.12,.18]);}
      else if(role==='policy'){for(let j=0;j<3;j++)prism([x+j*.02,.18+j*.12,z],[[0,-.31],[.36,0],[0,.31],[-.36,0]],.07);}
      else if(role==='human'){roundedBox([x,.12,z],[.38,.10,.24],.035);ring([x,.43,z],.24,.045,true);}
      else if(role==='authority'){ring([x,.2,z],.31,.055);cylinder([x,.09,z],[x,.16,z],.23,6);}
      else if(role==='executor'){roundedBox([x-.29,.28,z],[.13,.4,.16]);roundedBox([x+.29,.28,z],[.13,.4,.16]);roundedBox([x,.51,z],[.72,.1,.16]);roundedBox([x,.29,z],[.54,.055,.10]);}
      else if(role==='tool'){prism([x,.17,z],[[-.39,-.25],[.26,-.25],[.39,-.12],[.39,.25],[-.39,.25]],.16);roundedBox([x,.29,z],[.60,.035,.34],.025);}
      else {roundedBox([x,.34,z],[.57,.5,.09],.035);roundedBox([x+.18,.58,z+.01],[.16,.08,.12],.015);}
    });
    group('detail:'+node.id,[.06,.10,.13],()=>{
      if(role==='agent')roundedBox([x-.11,.417,z],[.36,.025,.07],.012);
      if(role==='authority')roundedBox([x,.172,z],[.22,.025,.045],.01);
      if(role==='evidence'){roundedBox([x,.40,z+.058],[.38,.025,.018],.01);roundedBox([x-.04,.27,z+.058],[.30,.025,.018],.01);}
    });
    group('block:'+node.id,[1,.25,.23],()=>{for(const sign of [-1,1])cylinder([x-.10,.88,z+sign*.10],[x+.10,.88,z-sign*.10],.025,12);});
  }
  group('packet',[1,.71,.25],()=>sphere([0,0,0],.075));
  return { values:new Float32Array(values),groups };
}

const vertexSource='attribute vec3 position; attribute vec3 normal; uniform mat4 camera; uniform vec3 offset; varying vec3 n; varying vec3 world; void main(){world=position+offset;n=normal;gl_Position=camera*vec4(world,1.0);}';
const fragmentSource='precision mediump float; uniform vec3 tint; uniform vec3 eye; varying vec3 n; varying vec3 world; void main(){vec3 normal=normalize(n);vec3 light=normalize(vec3(-0.4,0.85,0.6));float diffuse=max(dot(normal,light),0.0);vec3 view=normalize(eye-world);float spec=pow(max(dot(normal,normalize(light+view)),0.0),24.0);vec3 color=tint*(0.60+0.40*diffuse)+vec3(0.09*spec);gl_FragColor=vec4(color,1.0);}';

export function createFlowScene(canvas,{onUnavailable=()=>{},onProject=()=>{},topology=null}={}) {
  let gl,program,buffer,disposed=false,lost=false,lastProjection=null;
  let graph=topology,geometry;
  const upload=()=>{geometry=buildSceneGeometry(graph??{});gl.bindBuffer(gl.ARRAY_BUFFER,buffer);gl.bufferData(gl.ARRAY_BUFFER,geometry.values,gl.STATIC_DRAW);lastProjection=null;};
  const fail=()=>{if(!disposed){lost=true;onUnavailable('3D graphics unavailable. Use the complete 2D/list view.');}};
  const remove=()=>{if(buffer)gl?.deleteBuffer(buffer);if(program)gl?.deleteProgram(program);buffer=null;program=null;};
  try {
    gl=canvas.getContext('webgl',{alpha:true,antialias:true,depth:true,preserveDrawingBuffer:false});
    if(!gl)throw Error('unavailable');
    const shader=(kind,source)=>{const s=gl.createShader(kind);if(!s)throw Error('shader');gl.shaderSource(s,source);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS)){gl.deleteShader(s);throw Error('shader');}return s;};
    const vs=shader(gl.VERTEX_SHADER,vertexSource);let fs;
    try {fs=shader(gl.FRAGMENT_SHADER,fragmentSource);program=gl.createProgram();if(!program)throw Error('program');gl.attachShader(program,vs);gl.attachShader(program,fs);gl.linkProgram(program);if(!gl.getProgramParameter(program,gl.LINK_STATUS))throw Error('program');} finally {gl.deleteShader(vs);if(fs)gl.deleteShader(fs);}
    buffer=gl.createBuffer();if(!buffer)throw Error('buffer');
    upload();gl.useProgram(program);
    for(const [name,offset] of [['position',0],['normal',12]]){const index=gl.getAttribLocation(program,name);if(index<0)throw Error('attribute');gl.enableVertexAttribArray(index);gl.vertexAttribPointer(index,3,gl.FLOAT,false,24,offset);}
    const locations=Object.fromEntries(['camera','offset','tint','eye'].map(name=>[name,gl.getUniformLocation(program,name)]));
    if(Object.values(locations).some(value=>value===null))throw Error('uniform');
    gl.enable(gl.DEPTH_TEST);
    const lostHandler=event=>{event.preventDefault?.();fail();};canvas.addEventListener('webglcontextlost',lostHandler);
    return {
      invalidateProjection(){lastProjection=null;},
      setTopology(next){if(disposed||lost)return false;graph=next;upload();return true;},
      draw({camera=defaultCamera,frame=null,theme='dark',activeNodeId=null,blockedNodeId=null,unknownNodeId=null,motion=null,progress=0,selectedEdgeId=null}={}) {
        if(disposed||lost)return false;
        const width=Math.max(1,Math.min(2048,Math.round(canvas.clientWidth||800))),height=Math.max(1,Math.min(1200,Math.round(canvas.clientHeight||500)));
        const dpr=clamp(canvas.ownerDocument?.defaultView?.devicePixelRatio,1,1.5,1);
        const pixelsW=Math.min(2048,Math.round(width*dpr)),pixelsH=Math.min(1200,Math.round(height*dpr));
        if(canvas.width!==pixelsW)canvas.width=pixelsW;if(canvas.height!==pixelsH)canvas.height=pixelsH;
        gl.viewport(0,0,pixelsW,pixelsH);gl.clearColor(0,0,0,0);gl.clear(gl.COLOR_BUFFER_BIT|gl.DEPTH_BUFFER_BIT);gl.useProgram(program);
        let section=normalizeSectionFrame(frame);
        if(section && (!graph?.poses || section.nodeIds.some(id=>!graph.poses.some(pose=>pose.id===id))))section=null;
        let transform=cameraMatrix(camera,width/height,1,section);
        if(graph?.autoFitLabels&&graph.poses.length) {
          const fitPoses=section?graph.poses.filter(node=>section.nodeIds.includes(node.id)):graph.poses;
          let framing=1;
          for(let attempt=0;attempt<3;attempt++) {
            const projected=fitPoses.map(node=>projectPoint(add(node.position,[0,modelTops[node.kind]+.14-node.position[1],0]),transform.matrix));
            const extent=Math.max(...projected.map(p=>p?Math.abs(p.x-50)*width/100:width));
            const available=Math.max(30,width/2-72);
            const vertical=section?Math.max(...projected.map(p=>p?Math.abs(p.y-50)*height/100:height)):0;
            const verticalAvailable=Math.max(30,height/2-48);
            if(extent<=available&&vertical<=verticalAvailable)break;
            const correction=Math.min(available/Math.max(extent,1),section?verticalAvailable/Math.max(vertical,1):1);
            framing=Math.max(.4,framing*correction*.97);transform=cameraMatrix(camera,width/height,framing,section);
          }
        }
        gl.uniformMatrix4fv(locations.camera,false,new Float32Array(transform.matrix));gl.uniform3fv(locations.eye,transform.eye);
        const packet=motion?(graph?.pointOnEdge??pointOnSceneEdge)(motion.edgeId,progress):null;
        for(const group of geometry.groups){
          if(group.id.startsWith('block:')&&group.id!=='block:'+blockedNodeId)continue;
          if(group.id==='packet'&&!packet)continue;
          let tint=group.color;
          if(group.id==='edge:'+selectedEdgeId)tint=[1,.64,.26];
          if(group.id==='base:'+activeNodeId)tint=[.2,.65,.7];
          if(group.id==='base:'+blockedNodeId)tint=[.7,.16,.12];
          if(group.id==='base:'+unknownNodeId)tint=[.7,.5,.13];
          if(theme==='light'&&group.id.startsWith('base:'))tint=tint.map(v=>.6+v*.45);
          gl.uniform3fv(locations.tint,tint);gl.uniform3fv(locations.offset,group.id==='packet'?add(packet,[0,.12,0]):group.id==='node:'+activeNodeId?[0,.04,0]:[0,0,0]);gl.drawArrays(gl.TRIANGLES,group.start,group.count);
        }
        const key=JSON.stringify([normalizeCamera(camera),section,width,height]);
        if(key!==lastProjection){lastProjection=key;onProject((graph?.poses??SCENE_POSES).map(node=>({id:node.id,...projectPoint(add(node.position,[0,modelTops[node.kind??node.id]+.14-node.position[1],0]),transform.matrix)})));}
        return true;
      },
      dispose(){if(disposed)return;disposed=true;canvas.removeEventListener('webglcontextlost',lostHandler);remove();},
    };
  } catch { remove();fail();return null; }
}
