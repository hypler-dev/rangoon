import {createServer} from 'node:http';
import {readFile,stat,realpath} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {pathToFileURL} from 'node:url';
export function createSiteServer(directory){
 const rootReady=realpath(resolve(directory));
 const types={'.html':'text/html; charset=utf-8','.css':'text/css; charset=utf-8','.js':'text/javascript; charset=utf-8','.json':'application/json; charset=utf-8','.txt':'text/plain; charset=utf-8','.md':'text/markdown; charset=utf-8','.xml':'application/xml; charset=utf-8','.png':'image/png','.webp':'image/webp','.svg':'image/svg+xml','.woff2':'font/woff2'};
 return createServer(async(req,res)=>{
  const headers={'X-Content-Type-Options':'nosniff','Referrer-Policy':'strict-origin-when-cross-origin','X-Frame-Options':'DENY','Content-Security-Policy':"default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'none'",'Permissions-Policy':'camera=(), microphone=(), geolocation=()'};
  const send=(code,body,type='text/plain; charset=utf-8',extra={})=>{res.writeHead(code,{...headers,'Content-Type':type,...extra});res.end(req.method==='HEAD'?undefined:body);};
  let root;
  try{root=await rootReady;}catch{return send(503,'Site unavailable');}
  if(!['GET','HEAD'].includes(req.method))return send(405,'Method not allowed',undefined,{'Allow':'GET, HEAD'});
  let pathname;
  try{pathname=decodeURIComponent(new URL(req.url,'http://localhost').pathname);}catch{return send(400,'Bad request');}
  if(pathname.includes('\0')||pathname.includes('\\'))return send(400,'Bad request');
  if(pathname==='/.health')return send(200,'ok');
  let file=resolve(root,'.'+pathname);
  if(file!==root&&!file.startsWith(root+sep))return send(403,'Forbidden');
  if(pathname.split('/').some(segment=>segment.startsWith('.')&&segment!=='.well-known'&&segment!==''))return send(404,'Not found');
  try{
   const info=await stat(file);
   if(info.isDirectory()){
    if(!pathname.endsWith('/'))return send(308,'Redirect',undefined,{'Location':pathname+'/'+new URL(req.url,'http://localhost').search});
    file=resolve(file,'index.html');
   }
   const resolved=await realpath(file);
   if(!resolved.startsWith(root+sep))return send(403,'Forbidden');
   const body=await readFile(file); const type=types[extname(file)]||'application/octet-stream';
   const cache=pathname.startsWith('/assets/')?'public, max-age=3600':'no-cache';
   const links=[`</llms.txt>; rel="describedby"`];
   if(type.startsWith('text/html'))links.push(`<${pathname.endsWith('/')?pathname:pathname.replace(/index\.html$/,'')}index.md>; rel="alternate"; type="text/markdown"`);
   send(200,body,type,{'Cache-Control':cache,'Content-Length':body.length,'Link':links.join(', ')});
  }catch(error){
   if(error.code&&!['ENOENT','ENOTDIR'].includes(error.code))return send(500,'Internal server error');
   try{return send(404,await readFile(resolve(root,'404.html')),'text/html; charset=utf-8',{'Cache-Control':'no-cache'});}catch{return send(404,'Not found');}
  }
 });
}
if(process.argv[1]&&import.meta.url===pathToFileURL(await realpath(resolve(process.argv[1]))).href){
 const port=Number(process.env.PORT||4321);const host=process.env.HOST||'127.0.0.1';
 const directory=process.env.SITE_ROOT||new URL('../dist/',import.meta.url).pathname;
 const server=createSiteServer(directory);
 server.listen(port,host,()=>console.log(`Rangoon website: http://${host}:${server.address().port}`));
}
