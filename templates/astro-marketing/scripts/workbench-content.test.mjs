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
  ['sequence item comments on append', 'description: Kept\ntags:\n  - one # preserve one\n  - two # preserve two\n', {tags:['one','two','three']}],
]) {
  test(`patch safely rejects replacing ${name}`, () => {
    const source='---\ntitle: Post\npubDate: 2026-10-11\n'+frontmatter+'---\nBody';
    const run=spawnSync(process.execPath,['scripts/workbench-content.mjs'],{cwd:root,input:JSON.stringify({op:'patch',collection:'blog',source,fields}),encoding:'utf8'});
    assert.notEqual(run.status,0);
    assert.match(run.stderr,/cannot safely.*comment/i);
    assert.equal(run.stdout,'');
  });
}
