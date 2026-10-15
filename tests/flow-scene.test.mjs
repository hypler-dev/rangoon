import test from 'node:test';
import assert from 'node:assert/strict';
import { buildSceneGeometry, cameraMatrix, projectPoint, normalizeCamera, SCENE_POSES, createFlowScene } from '../preview/flow-scene.mjs';

test('finite bounded camera projects eight distinct volume positions and rejects invalid points',()=>{
  assert.deepEqual(normalizeCamera({yaw:Infinity,pitch:1000,zoom:-100}),{yaw:-12,pitch:52,zoom:.85});
  for(const camera of [{},{yaw:-24,pitch:52,zoom:.8},{yaw:24,pitch:32,zoom:1.25}]) {
    const {matrix}=cameraMatrix(camera,1.5);
    const projected=SCENE_POSES.map(node=>projectPoint(node.position,matrix));
    assert.ok(projected.every(p=>p&&Number.isFinite(p.x)&&Number.isFinite(p.y)&&p.visible));
    assert.equal(new Set(projected.map(p=>p.x+','+p.y)).size,8);
  }
  assert.equal(projectPoint([NaN,0,0],cameraMatrix({},1).matrix),null);
  assert.equal(projectPoint([0,0,0],[1]),null);
});

test('closed volume meshes contain real depth, bounded triangles and distinct system geometry',()=>{
  const geometry=buildSceneGeometry();assert.ok(geometry.values.length/6<100000);
  assert.ok(Array.from(geometry.values).every(Number.isFinite));
  assert.equal(geometry.groups.filter(g=>g.id.startsWith('node:')).length,8);
  assert.equal(geometry.groups.filter(g=>g.id.startsWith('edge:')).length,10);
  for(const g of geometry.groups) {assert.equal(g.count%3,0);assert.ok(g.start+g.count<=geometry.values.length/6);}
  const agent=geometry.groups.find(g=>g.id==='node:agent');
  const coordinates=Array.from(geometry.values.slice(agent.start*6,(agent.start+agent.count)*6));
  assert.ok(Math.max(...coordinates.filter((_,i)=>i%6===1))-Math.min(...coordinates.filter((_,i)=>i%6===1))>.2);
});

function fakeCanvas({compile=true,context=true}={}) {
  const calls=[],events=new Map();let uniform=0;
  const gl={ARRAY_BUFFER:1,STATIC_DRAW:2,VERTEX_SHADER:3,FRAGMENT_SHADER:4,COMPILE_STATUS:5,LINK_STATUS:6,FLOAT:7,DEPTH_TEST:8,COLOR_BUFFER_BIT:16,DEPTH_BUFFER_BIT:32,TRIANGLES:9,
    createShader:()=>({}),shaderSource(){},compileShader(){},getShaderParameter:()=>compile,deleteShader:()=>calls.push('deleteShader'),createProgram:()=>({}),attachShader(){},linkProgram(){},getProgramParameter:()=>true,deleteProgram:()=>calls.push('deleteProgram'),createBuffer:()=>({}),deleteBuffer:()=>calls.push('deleteBuffer'),bindBuffer(){},bufferData:(target,data)=>calls.push(['bufferData',data.length]),useProgram(){},getAttribLocation:()=>0,enableVertexAttribArray(){},vertexAttribPointer(){},getUniformLocation:()=>++uniform,enable(){},viewport:(...args)=>calls.push(['viewport',...args]),clearColor(){},clear(){},uniformMatrix4fv:(location,transpose,matrix)=>calls.push(['matrix',Array.from(matrix)]),uniform3fv(){},drawArrays:()=>calls.push('draw')};
  const canvas={width:0,height:0,clientWidth:10000,clientHeight:5000,ownerDocument:{defaultView:{devicePixelRatio:10}},getContext:()=>context?gl:null,addEventListener:(type,fn)=>events.set(type,fn),removeEventListener:type=>events.delete(type)};
  return {canvas,calls,events};
}

test('GPU buffers built once, drawing caps pixels, disposal fences late work and frees resources',()=>{
  const f=fakeCanvas();let projections=0;
  const scene=createFlowScene(f.canvas,{onProject:()=>projections++});assert.ok(scene);
  scene.draw();scene.draw({motion:{x:250,y:150}});scene.draw({camera:{yaw:10}});
  assert.equal(f.calls.filter(x=>Array.isArray(x)&&x[0]==='bufferData').length,1);
  assert.ok(f.canvas.width<=2048&&f.canvas.height<=1200);
  assert.equal(projections,2);
  scene.dispose();const count=f.calls.length;scene.dispose();assert.equal(scene.draw(),false);
  assert.equal(f.calls.length,count);assert.equal(f.events.size,0);
  assert.equal(f.calls.filter(x=>x==='deleteBuffer').length,1);assert.equal(f.calls.filter(x=>x==='deleteProgram').length,1);
});

test('missing GPU, shader failure and lost context return honest fallback and stop drawing',()=>{
  for(const options of [{context:false},{compile:false}]){const f=fakeCanvas(options);let reason;assert.equal(createFlowScene(f.canvas,{onUnavailable:v=>reason=v}),null);assert.match(reason,/unavailable/);assert.equal(f.events.size,0);}
  const f=fakeCanvas();let failures=0;const scene=createFlowScene(f.canvas,{onUnavailable:()=>failures++});let prevented=false;
  f.events.get('webglcontextlost')({preventDefault(){prevented=true;}});const count=f.calls.length;
  assert.equal(prevented,true);assert.equal(failures,1);assert.equal(scene.draw(),false);assert.equal(f.calls.length,count);scene.dispose();
});

test('route sockets attach to smaller volume pads and packets share that exact 3D path', async()=>{
  const {sceneRoute,pointOnSceneEdge}=await import('../preview/flow-scene.mjs');
  const {FLOW_EDGES}=await import('../preview/flow-simulation.mjs');
  for(const edge of FLOW_EDGES){const route=sceneRoute(edge.id),from=SCENE_POSES.find(n=>n.id===edge.from).position,to=SCENE_POSES.find(n=>n.id===edge.to).position;
    assert.ok(Math.abs(Math.hypot(route[0][0]-from[0],route[0][2]-from[2])-.56)<1e-8);
    assert.ok(Math.abs(Math.hypot(route.at(-1)[0]-to[0],route.at(-1)[2]-to[2])-.56)<1e-8);
    assert.deepEqual(pointOnSceneEdge(edge.id,0),route[0]);assert.deepEqual(pointOnSceneEdge(edge.id,1),route.at(-1));
  }
  assert.equal(pointOnSceneEdge('untrusted',.5),null);assert.equal(pointOnSceneEdge('request',Infinity),null);
});

test('section frames translate and magnify at fixed orientation, with invalid frames preserving legacy projection',()=>{
  const overview=cameraMatrix({},1.5), frame={center:[1,.35,-.5],framing:1.6,nodeIds:['agent']};
  const section=cameraMatrix({},1.5,1,frame);
  assert.notDeepEqual(section.matrix,overview.matrix);
  const target=projectPoint(frame.center,section.matrix);
  assert.ok(Math.abs(target.x-50)<1e-9&&Math.abs(target.y-50)<1e-9);
  const direction=section.eye.map((value,index)=>value-frame.center[index]);
  const length=Math.hypot(...direction), oldLength=Math.hypot(...overview.eye);
  direction.forEach((value,index)=>assert.ok(Math.abs(value/length-overview.eye[index]/oldLength)<1e-12));
  assert.ok(length<oldLength);
  for(const bad of [null,{}, {...frame,center:[Infinity,0,0]}, {...frame,center:[13,0,0]}, {...frame,framing:100}, {...frame,nodeIds:[]}, {...frame,nodeIds:['agent','agent']}, {...frame,nodeIds:['<script>']}]) assert.deepEqual(cameraMatrix({},1.5,1,bad),overview);
});

test('section changes refresh every label projection without rebuilding GPU buffers; missing IDs fall back',()=>{
  const f=fakeCanvas();f.canvas.clientWidth=1000;f.canvas.clientHeight=430;
  const poses=[{id:'left',kind:'agent',position:[-2,.15,0]},{id:'right',kind:'tool',position:[2,.15,1]}];
  const topology={poses,edges:[],autoFitLabels:true,route:()=>null};
  const projections=[];const scene=createFlowScene(f.canvas,{topology,onProject:value=>projections.push(value)});
  scene.draw();const first=f.calls.filter(value=>Array.isArray(value)&&value[0]==='matrix').at(-1)[1];
  const frame={center:[-2,.35,0],framing:1.6,nodeIds:['left']};scene.draw({frame});scene.draw({frame});
  assert.equal(projections.length,2);
  assert.deepEqual(projections[1].map(value=>value.id),['left','right']);
  assert.notDeepEqual(projections[1],projections[0]);
  assert.ok(projections[1][0].visible);
  scene.draw({frame:{...frame,nodeIds:['missing']}});
  assert.equal(projections.length,3);
  assert.deepEqual(f.calls.filter(value=>Array.isArray(value)&&value[0]==='matrix').at(-1)[1],first);
  assert.equal(f.calls.filter(value=>Array.isArray(value)&&value[0]==='bufferData').length,1);
  scene.draw({frame});scene.dispose();assert.equal(scene.draw({frame}),false);
});
