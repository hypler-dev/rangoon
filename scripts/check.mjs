import {readFile,readdir,stat} from 'node:fs/promises';
import {resolve} from 'node:path';
import assert from 'node:assert/strict';
const root=resolve('dist');const routes=JSON.parse(await readFile(resolve(root,'search.json'),'utf8'));
const errors=[];let checked=0;
for(const page of routes){
 const html=await readFile(resolve(root,'.'+page.path,'index.html'),'utf8');
 for(const required of ['<title>','name="description"','rel="canonical"','rel="alternate"','id="main"'])if(!html.includes(required))errors.push(`${page.path}: missing ${required}`);
 if((html.match(/<h1[ >]/g)||[]).length!==1)errors.push(`${page.path}: expected one h1`);
 if(!html.includes('https://github.com/hypler-dev/rangoon'))errors.push(`${page.path}: missing repository`);
 for(const [,url] of html.matchAll(/(?:href|src|data-image)="([^"#]+)(?:#[^"]*)?"/g)){
  if(!url.startsWith('/'))continue;
  try{const target=resolve(root,'.'+url.split('?')[0]);const info=await stat(target);if(info.isDirectory())await stat(resolve(target,'index.html'));checked++;}catch{errors.push(`${page.path}: missing ${url}`);}
 }
 await stat(resolve(root,'.'+page.path,'index.md'));
}
for(const p of ['llms.txt','llms-full.txt','ai.txt','agents.txt','robots.txt','sitemap.xml','project.json','.well-known/ai-policy.json'])await stat(resolve(root,p));
JSON.parse(await readFile(resolve(root,'.well-known/ai-policy.json'),'utf8'));
JSON.parse(await readFile(resolve(root,'project.json'),'utf8'));
const llms=await readFile(resolve(root,'llms.txt'),'utf8');
for(const [,url]of llms.matchAll(/\]\(https:\/\/rangoon\.ai([^)]*)\)/g)){try{await stat(resolve(root,'.'+url));checked++;}catch{errors.push(`llms.txt: missing ${url}`);}}
assert.equal(new Set(routes.map(p=>p.path)).size,routes.length,'Duplicate routes');
if(errors.length){console.error(errors.join('\n'));process.exit(1);}
console.log(`PASS: ${routes.length} pages, ${checked} local references, Markdown companions, SEO metadata, sitemap, and AI resources.`);
