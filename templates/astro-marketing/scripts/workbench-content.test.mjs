import assert from 'node:assert/strict';
import { test } from 'node:test';
import { spawnSync } from 'node:child_process';
import { z } from 'astro/zod';
import { createCollectionSchemas } from '../src/content-schemas.mjs';

const root = new URL('../', import.meta.url);
function helper(request) {
  const run = spawnSync(process.execPath, ['scripts/workbench-content.mjs'], {cwd: root, input: JSON.stringify(request), encoding:'utf8'});
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}
const post = {title:'Post',description:'Description',pubDate:'2026-10-11'};
test('describe identifies dates, image controls, references and section variants', () => {
  const {collections} = helper({op:'describe'});
  assert.deepEqual(collections.map(({name,loader})=>({name,loader})), helper({op:'list'}).collections.map(({name,loader})=>({name,loader})));
  const blog = collections.find(c=>c.name==='blog').fields;
  assert.equal(blog.properties.pubDate.type,'date');
  assert.equal(blog.properties.coverImage.type,'image');
  assert.equal(blog.properties.relatedPosts.items.collection,'blog');
  assert.equal(collections.find(c=>c.name==='pages').fields.properties.sections.items.collection,'sections');
  assert.equal(collections.find(c=>c.name==='sections').fields.discriminator,'type');
  assert.equal(collections.find(c=>c.name==='sections').fields.variants.length,7);
});
test('validate uses live schemas and returns defaults and precise issue paths', () => {
  const schemas = createCollectionSchemas({z});
  for (const [collection,fields] of [['blog',post],['blog',{...post,pubDate:'2026-02-30'}],['blog',{...post,coverImage:'./cover.png'}],['helpDocs',{title:'Guide',description:'Help',section:'Setup',order:1}],['helpDocs',{title:'Guide',description:'Help',section:'Setup',order:'1'}]]) {
    const expected=schemas[collection].safeParse(fields);
    const actual=helper({op:'validate',collection,fields});
    assert.equal(actual.valid,expected.success);
    if(expected.success) assert.deepEqual(actual.fields,expected.data);
    else assert.deepEqual(actual.errors,expected.error.issues.map(({path,message})=>({path,message})));
  }
  assert.deepEqual(helper({op:'validate',collection:'blog',fields:{...post,coverImage:'./cover.png'}}).errors.map(e=>e.path),[['coverAlt']]);
});
test('patch changes YAML values while retaining quotes, comments, CRLF and body bytes', () => {
  const source="---\r\n# heading\r\ntitle: 'Old' # keep\r\ndescription: \"Unchanged\"\r\npubDate: 2026-10-11\r\ntags: [one, two] # tags\r\n---\r\n\r\nBody  \r\n";
  assert.equal(helper({op:'patch',collection:'blog',source,fields:{title:'New'}}).source,source.replace("'Old'","'New'"));
  assert.equal(helper({op:'patch',collection:'blog',source,fields:{title:'Old'}}).source,source);
  assert.equal(helper({op:'patch',collection:'blog',source,fields:{},body:'Changed\n'}).source,source.slice(0,source.indexOf('\r\nBody')-0)+'Changed\n');
});
test('patch preserves unrelated nested JSON formatting and escaped key spelling', () => {
  const source='{\r\n  "na\\u006de" : "Acme",\r\n  "footer": { "text" : "Old", "links": [] },\r\n  "nav" : [],\r\n  "description": "Kept"\r\n}\r\n';
  assert.equal(helper({op:'patch',collection:'site',source,fields:{footer:{text:'New',links:[]}}}).source,source.replace('"Old"','"New"'));
  assert.equal(helper({op:'patch',collection:'site',source,fields:{name:'Acme'}}).source,source);
});
test('unknown transforms and custom refinements are read-only descriptors', async () => {
  const module = await import('./workbench-content.mjs');
  assert.equal(typeof module.describeSchema,'function');
  for (const schema of [z.string().transform(v=>v.toUpperCase()),z.string().refine(v=>v==='x')]) {
    const descriptor=module.describeSchema(schema);
    assert.equal(descriptor.type,'unsupported');
    assert.match(descriptor.reason,/transform|refinement/i);
  }
  assert.equal(module.describeSchema(z.enum(['one','two'])).type,'enum');
  assert.equal(module.describeSchema(z.number()).type,'number');
});
test('helper supports line-delimited requests', () => {
  const run=spawnSync(process.execPath,['scripts/workbench-content.mjs'],{cwd:root,input:'{"op":"list"}\n{"op":"describe"}\n',encoding:'utf8'});
  assert.equal(run.status,0,run.stderr);
  assert.equal(run.stdout.trim().split('\n').length,2);
});
test('patch validates the complete merged candidate before returning source', () => {
  const source='---\ntitle: Old\ndescription: Kept\npubDate: 2026-10-11\n---\nBody';
  const run=spawnSync(process.execPath,['scripts/workbench-content.mjs'],{cwd:root,input:JSON.stringify({op:'patch',collection:'blog',source,fields:{pubDate:'2026-02-30'}}),encoding:'utf8'});
  assert.notEqual(run.status,0);
  assert.match(run.stderr,/pubDate/);
});
test('YAML insertion and nested edits preserve untouched nodes', () => {
  const source='---\ntype: hero\nheadline: "Hello" # retain\ncta:\n  label: \'Old\' # label\n  href: /start # retain\n---\n<Widget />\n';
  assert.equal(helper({op:'patch',collection:'sections',source,fields:{cta:{label:'New',href:'/start'}}}).source,source.replace("'Old'","'New'"));
  const inserted=helper({op:'patch',collection:'sections',source,fields:{subheadline:'Added'}}).source;
  assert.ok(inserted.includes(source.slice(0,source.indexOf('---\n<Widget'))));
  assert.ok(inserted.endsWith('---\n<Widget />\n'));
});
test('changing a block scalar retains the following field and supports multiline text', () => {
  const source='---\ntitle: Old\ndescription: |\n  Old line\n  Other line\npubDate: 2026-10-11 # keep\n---\nBody';
  const result=helper({op:'patch',collection:'blog',source,fields:{description:'New\nText'}}).source;
  assert.ok(result.includes('pubDate: 2026-10-11 # keep\n---\nBody'));
});
test('Astro builds and helper validation agree for valid and invalid Blog and Help entries', async () => {
  const {cpSync,mkdtempSync,symlinkSync,writeFileSync,rmSync,readdirSync}=await import('node:fs');
  const {tmpdir}=await import('node:os');
  const {join}=await import('node:path');
  const {fileURLToPath}=await import('node:url');
  const dir=mkdtempSync(join(tmpdir(),'helper-build-'));
  try {
    for(const path of ['src','astro.config.mjs','package.json']) cpSync(new URL(path,root),join(dir,path),{recursive:true});
    symlinkSync(fileURLToPath(new URL('node_modules',root)),join(dir,'node_modules'),'dir');
    const run=()=>spawnSync(process.execPath,[fileURLToPath(new URL('node_modules/astro/bin/astro.mjs',root)),'build'],{cwd:dir,encoding:'utf8'});
    let result=run();
    assert.equal(result.status,0,result.stdout+result.stderr);
    // These schemas are generated from the actual content.config.ts collection wiring.
    assert.deepEqual(readdirSync(join(dir,'.astro/collections')).map(name=>name.replace('.schema.json','')).sort(),helper({op:'list'}).collections.map(c=>c.name).sort());
    for(const [collection,fields,field] of [
      ['blog',{...post,pubDate:'2026-02-30'},'pubDate'],
      ['blog',{...post,coverImage:'https://example.com/cover.png'},'coverAlt'],
      ['helpDocs',{title:'Guide',description:'Help',section:'Setup',order:'wrong'},'order'],
    ]) {
      const path=join(dir,'src/content',collection,'en','invalid.md');
      writeFileSync(path,'---\n'+Object.entries(fields).map(([key,value])=>`${key}: ${JSON.stringify(value)}`).join('\n')+'\n---\nBody');
      assert.equal(helper({op:'validate',collection,fields}).valid,false);
      result=run();
      assert.notEqual(result.status,0);
      assert.ok((result.stdout+result.stderr).includes(field),result.stdout+result.stderr);
      rmSync(path);
    }
  } finally {rmSync(dir,{recursive:true,force:true});}
});
for (const [name, frontmatter, fields] of [
  ['block scalar header comment', 'description: | # preserve this comment\n  Old line\n', {description:'New'}],
  ['a commented sequence item', 'description: Kept\ntags:\n  - one # preserve one\n  - two # preserve two\n', {tags:['two']}],
]) {
  test(`patch safely rejects replacing ${name}`, () => {
    const source='---\ntitle: Post\npubDate: 2026-10-11\n'+frontmatter+'---\nBody';
    const run=spawnSync(process.execPath,['scripts/workbench-content.mjs'],{cwd:root,input:JSON.stringify({op:'patch',collection:'blog',source,fields}),encoding:'utf8'});
    assert.notEqual(run.status,0);
    assert.match(run.stderr,/cannot safely.*comment/i);
    assert.equal(run.stdout,'');
  });
}
test('appending to a list keeps the comments on existing items', () => {
  const source='---\ntitle: Post\npubDate: 2026-10-11\ndescription: Kept\ntags:\n  - one # preserve one\n  - two # preserve two\n---\nBody';
  assert.equal(helper({op:'patch',collection:'blog',source,fields:{tags:['one','two','three']}}).source,source.replace('two # preserve two\n','two # preserve two\n  - three\n'));
});
test('patch unset removes optional top-level fields while preserving surrounding source', () => {
  const source="---\r\ntitle: 'Post' # keep\r\ndescription: Kept\r\npubDate: 2026-10-11\r\ncoverImage: ./cover.png\r\ncoverAlt: Cover\r\ntags: [one] # tags\r\n---\r\nBody\r\n";
  assert.equal(helper({op:'patch',collection:'blog',source,fields:{},unset:['coverImage','coverAlt']}).source,source.replace('coverImage: ./cover.png\r\ncoverAlt: Cover\r\n',''));
  assert.equal(helper({op:'patch',collection:'blog',source,fields:{},unset:['missing']}).source,source);
  const json='{\n  "name": "Acme",\n  "description": "Kept",\n  "nav" : [],\n  "footer": { "text": "F" },\n  "socials": []\n}\n';
  assert.equal(helper({op:'patch',collection:'site',source:json,fields:{},unset:['socials']}).source,'{\n  "name": "Acme",\n  "description": "Kept",\n  "nav" : [],\n  "footer": { "text": "F" }\n}\n');
});
test('patch unset validates the candidate and rejects malformed or overlapping names', () => {
  const source='---\ntitle: Post\ndescription: Kept\npubDate: 2026-10-11\n---\nBody';
  for (const [request, pattern] of [
    [{unset:['title']}, /title/],
    [{unset:'title'}, /unset/],
    [{unset:[['title']]}, /unset/],
    [{unset:['title'],fields:{title:'New'}}, /unset/],
  ]) {
    const run=spawnSync(process.execPath,['scripts/workbench-content.mjs'],{cwd:root,input:JSON.stringify({op:'patch',collection:'blog',source,fields:{},...request}),encoding:'utf8'});
    assert.notEqual(run.status,0);
    assert.match(run.stderr,pattern);
    assert.equal(run.stdout,'');
  }
});
test('adding and removing a site nav link preserves the surrounding JSON bytes', async () => {
  const {readFileSync}=await import('node:fs');
  const source=readFileSync(new URL('src/content/site.json',root),'utf8');
  const site=JSON.parse(source);
  const added=helper({op:'patch',collection:'site',source,fields:{nav:[...site.nav,{label:'Docs',href:'/docs/'}]}}).source;
  assert.equal(added,source.replace('{ "label": "Help", "href": "/help/" }','{ "label": "Help", "href": "/help/" },\n    { "label": "Docs", "href": "/docs/" }'));
  assert.equal(helper({op:'patch',collection:'site',source:added,fields:{nav:site.nav}}).source,source);
  const removed=helper({op:'patch',collection:'site',source,fields:{nav:site.nav.filter(link=>link.label!=='FAQ')}}).source;
  assert.equal(removed,source.replace('    { "label": "FAQ", "href": "#faq" },\n',''));
  const first=helper({op:'patch',collection:'site',source,fields:{nav:site.nav.slice(1)}}).source;
  assert.equal(first,source.replace('    { "label": "Features", "href": "#features" },\n',''));
  const last=helper({op:'patch',collection:'site',source,fields:{nav:site.nav.slice(0,-1)}}).source;
  assert.equal(last,source.replace(',\n    { "label": "Help", "href": "/help/" }',''));
});
test('adding and removing a page section preserves the surrounding JSON bytes', async () => {
  const {readFileSync}=await import('node:fs');
  const source=readFileSync(new URL('src/content/pages/home.json',root),'utf8');
  const page=JSON.parse(source);
  const added=helper({op:'patch',collection:'pages',source,fields:{sections:[...page.sections,'hero']}}).source;
  assert.equal(added,source.replace('"cta"]','"cta", "hero"]'));
  const removed=helper({op:'patch',collection:'pages',source,fields:{sections:page.sections.filter(s=>s!=='pricing')}}).source;
  assert.equal(removed,source.replace('"pricing", ',''));
  const inserted=helper({op:'patch',collection:'pages',source,fields:{sections:['cta',...page.sections]}}).source;
  assert.equal(inserted,source.replace('["hero"','["cta", "hero"'));
  const pretty='{\n  "title": "T",\n  "description": "D",\n  "sections": [\n    "hero"\n  ]\n}\n';
  assert.equal(helper({op:'patch',collection:'pages',source:pretty,fields:{sections:['hero','faq']}}).source,pretty.replace('"hero"\n','"hero",\n    "faq"\n'));
  assert.equal(helper({op:'patch',collection:'pages',source:pretty,fields:{sections:[]}}).source,pretty.replace('[\n    "hero"\n  ]','[]'));
  const empty='{\n  "title": "T",\n  "description": "D",\n  "sections": []\n}\n';
  assert.equal(helper({op:'patch',collection:'pages',source:empty,fields:{sections:['hero']}}).source,empty.replace('[]','["hero"]'));
});
test('adding and removing a YAML FAQ item keeps block style and surrounding bytes', async () => {
  const {readFileSync}=await import('node:fs');
  const source=readFileSync(new URL('src/content/sections/faq.mdx',root),'utf8');
  const item={question:'Is there a free plan?',answer:'Yes.'};
  const added=helper({op:'patch',collection:'sections',source,fields:{items:[{question:'Can I change the design?',answer:'Yes. Everything is driven by files in the content folder and simple components.'},{question:'Where can I host it?',answer:'Anywhere that serves static files, including Cloudflare Pages.'},item]}}).source;
  assert.equal(added,source.replace('Cloudflare Pages.\n','Cloudflare Pages.\n  - question: Is there a free plan?\n    answer: Yes.\n'));
  const removed=helper({op:'patch',collection:'sections',source,fields:{items:[{question:'Where can I host it?',answer:'Anywhere that serves static files, including Cloudflare Pages.'}]}}).source;
  assert.equal(removed,source.replace('  - question: Can I change the design?\n    answer: Yes. Everything is driven by files in the content folder and simple components.\n',''));
  const back=helper({op:'patch',collection:'sections',source:added,fields:{items:[{question:'Can I change the design?',answer:'Yes. Everything is driven by files in the content folder and simple components.'},{question:'Where can I host it?',answer:'Anywhere that serves static files, including Cloudflare Pages.'}]}}).source;
  assert.equal(back,source);
});
test('YAML block and flow tag lists gain and lose items in their own style', () => {
  const block='---\ntitle: Post\ndescription: D\npubDate: "2026-10-11"\ntags:\n  - news\n  - launch\nrelatedPosts: []\n---\nBody';
  assert.equal(helper({op:'patch',collection:'blog',source:block,fields:{tags:['news','launch','x']}}).source,block.replace('  - launch\n','  - launch\n  - x\n'));
  assert.equal(helper({op:'patch',collection:'blog',source:block,fields:{tags:['launch']}}).source,block.replace('  - news\n',''));
  assert.equal(helper({op:'patch',collection:'blog',source:block,fields:{tags:[]}}).source,block.replace('tags:\n  - news\n  - launch\n','tags: []\n'));
  assert.equal(helper({op:'patch',collection:'blog',source:block,fields:{relatedPosts:['welcome']}}).source,block.replace('relatedPosts: []','relatedPosts: [welcome]'));
  const flow='---\ntitle: Post\ndescription: D\npubDate: "2026-10-11"\ntags: [news]\n---\nBody';
  assert.equal(helper({op:'patch',collection:'blog',source:flow,fields:{tags:['news','launch']}}).source,flow.replace('[news]','[news, launch]'));
  assert.equal(helper({op:'patch',collection:'blog',source:flow,fields:{tags:[]}}).source,flow.replace('[news]','[]'));
});
test('YAML lists whose comments cannot be preserved are refused with a Code hint', () => {
  const source='---\ntitle: Post\ndescription: D\npubDate: "2026-10-11"\ntags:\n  - news # first\n  - launch\n---\nBody';
  const run=spawnSync(process.execPath,['scripts/workbench-content.mjs'],{cwd:root,input:JSON.stringify({op:'patch',collection:'blog',source,fields:{tags:['launch']}}),encoding:'utf8'});
  assert.notEqual(run.status,0);
  assert.match(run.stderr,/edit this field in Code/);
});
test('date-like strings are written so Astro frontmatter parsing keeps them strings', async () => {
  const yaml=(await import('js-yaml')).default;
  const source='---\ntitle: Post\ndescription: D\npubDate: "2026-10-11"\ntags: []\n---\nBody';
  for (const fields of [{title:'2026-10-12'},{title:'true'},{tags:['2026-10-12']},{coverAlt:'2026-10-12'}]) {
    const clean=Object.fromEntries(Object.entries(fields).filter(([,v])=>v!==undefined));
    const result=helper({op:'patch',collection:'blog',source,fields:clean}).source;
    const parsed=yaml.load(result.split('---\n')[1]);
    for (const [key,value] of Object.entries(clean)) assert.deepEqual(parsed[key],value,result);
  }
  const block='---\ntitle: Post\ndescription: D\npubDate: "2026-10-11"\ntags:\n  - news\n---\nBody';
  const parsed=yaml.load(helper({op:'patch',collection:'blog',source:block,fields:{tags:['news','2026-10-12']}}).source.split('---\n')[1]);
  assert.deepEqual(parsed.tags,['news','2026-10-12']);
});
