import { collectionRegistry, createCollectionSchemas } from '../src/content-schemas.mjs';
import { z } from 'astro/zod';
import { parseDocument, isMap, isScalar, stringify } from 'yaml';
import { parseTree, findNodeAtLocation, modify, applyEdits } from 'jsonc-parser';
import { isDeepStrictEqual } from 'node:util';
import { pathToFileURL } from 'node:url';
import { createInterface } from 'node:readline';
const schemas = createCollectionSchemas({ z });
const bundled = new Set(['site', 'pages', 'sections', 'blog', 'helpDocs']);
const list = () => Object.entries(collectionRegistry).map(([name, definition]) => ({ name, ...definition, supportedForEditing: bundled.has(name) }));
/** A limited projection of live Zod schemas, never a validation schema. */
export function describeSchema(schema) {
  const def = schema._def, metadata = schema.meta?.()?.editor ?? {};
  if (def.type === 'optional' || def.type === 'default') return { ...describeSchema(def.innerType), required: false, ...(def.type === 'default' ? { default: def.defaultValue } : {}) };
  const base = { required: true };
  const unsupported = reason => ({ ...base, type: 'unsupported', reason });
  if (def.type === 'pipe' || def.type === 'transform') return unsupported('Unknown transform cannot be represented');
  if (def.checks?.some(check => check._zod?.def?.check === 'custom') && !metadata.knownRefinement) return unsupported('Unknown custom refinement cannot be represented');
  if (metadata.type) return { ...base, type: metadata.type, ...(metadata.collection ? { collection: metadata.collection } : {}) };
  switch (def.type) {
    case 'string': case 'boolean': case 'number': case 'date': return { ...base, type: def.type };
    case 'literal': return { ...base, type: 'literal', values: def.values };
    case 'enum': return { ...base, type: 'enum', values: Object.values(def.entries) };
    case 'array': return { ...base, type: 'array', items: describeSchema(def.element) };
    case 'object': return { ...base, type: 'object', properties: Object.fromEntries(Object.entries(def.shape).map(([name, field]) => [name, describeSchema(field)])) };
    case 'union':
      if (def.discriminator) return { ...base, type: 'union', discriminator: def.discriminator, variants: def.options.map(describeSchema) };
      return unsupported('Non-discriminated union cannot be represented');
    default: return unsupported(`Unsupported schema node: ${def.type}`);
  }
}
function blocked(field) {
  if (field.required === false) return false;
  if (field.type === 'unsupported') return true;
  return Object.values(field.properties ?? {}).some(blocked) || (field.items ? blocked(field.items) : false) || (field.variants ?? []).some(blocked);
}
function changes(old, next, path = [], result = []) {
  if (isDeepStrictEqual(old, next)) return result;
  if (old && next && typeof old === 'object' && typeof next === 'object' && !Array.isArray(old) && !Array.isArray(next)) {
    for (const key of new Set([...Object.keys(old), ...Object.keys(next)])) changes(old[key], next[key], [...path,key], result);
  } else if (Array.isArray(old) && Array.isArray(next) && old.length === next.length) next.forEach((value,index)=>changes(old[index],value,[...path,index],result));
  else result.push({path,value:next});
  return result;
}
function validateCandidate(collection, fields) {
  const result=schemas[collection].safeParse(fields);
  if (!result.success) throw new Error(JSON.stringify(result.error.issues.map(({path,message})=>({path,message}))));
}
function patchJson(source, fields, collection) {
  const previous = JSON.parse(source);
  if (!previous || Array.isArray(previous) || typeof previous !== 'object') throw new Error('Expected JSON object');
  validateCandidate(collection, {...previous,...fields});
  let result = source;
  for (const {path,value} of changes(previous, {...previous,...fields})) {
    const node = findNodeAtLocation(parseTree(result),path);
    if (node && value !== undefined) result = result.slice(0,node.offset)+JSON.stringify(value)+result.slice(node.offset+node.length);
    else result = applyEdits(result,modify(result,path,value,{}));
  }
  return result;
}
// Inspect syntax tokens rather than '#' text, which may be part of a scalar value.
function yamlComments(token, ranges = []) {
  if (!token || typeof token !== 'object') return ranges;
  if (token.type === 'comment') ranges.push({start:token.offset,end:token.offset+token.source.length});
  for (const child of Object.values(token)) {
    if (Array.isArray(child)) child.forEach(value=>yamlComments(value,ranges));
    else if (child && typeof child === 'object') yamlComments(child,ranges);
  }
  return ranges;
}
function patchYaml(source, fields, collection) {
  const document = parseDocument(source, { keepSourceTokens: true, uniqueKeys: true });
  if (document.errors.length) throw new Error(document.errors[0].message);
  if (!isMap(document.contents)) throw new Error('Expected frontmatter mapping');
  const previous = document.toJS({maxAliasCount:100});
  validateCandidate(collection, {...previous,...fields});
  const eol = source.includes('\r\n') ? '\r\n' : '\n';
  const edits = [];
  for (const {path,value} of changes(previous,{...previous,...fields})) {
    const node = document.getIn(path,true);
    if (node && node.range && value !== undefined) {
      let text;
      if (typeof value === 'string' && /[\r\n]/.test(value)) text = JSON.stringify(value);
      else if (isScalar(node) && typeof value === 'string' && node.type === 'QUOTE_SINGLE') text = `'${value.replaceAll("'","''")}'`;
      else if (isScalar(node) && typeof value === 'string' && node.type === 'QUOTE_DOUBLE') text = JSON.stringify(value);
      else text = stringify(value, { collectionStyle: 'flow', lineWidth: 0 }).trimEnd();
      const end = node.range[1];
      if (/\r?\n$/.test(source.slice(node.range[0],end))) text += eol;
      edits.push({start:node.range[0],end,text});
    } else {
      const parentPath=path.slice(0,-1), key=path.at(-1), parent=document.getIn(parentPath,true);
      if (!isMap(parent) || parent.flow) throw new Error('Cannot safely add or remove a field in this YAML shape');
      const pair=parent.items.find(pair=>pair.key?.value===key);
      if (value === undefined && pair) {
        const start=source.lastIndexOf('\n',pair.key.range[0]-1)+1, end=pair.value?.range?.[2] ?? pair.key.range[2];
        edits.push({start,end,text:''});
      } else if (!pair && value !== undefined) {
        const indent=parent.items.length ? parent.items[0].key.range[0]-(source.lastIndexOf('\n',parent.items[0].key.range[0]-1)+1) : 0, end=parent.range[2];
        edits.push({start:end,end,text:`${source.slice(0,end).endsWith('\n')?'':eol}${' '.repeat(indent)}${JSON.stringify(key)}: ${stringify(value,{collectionStyle:'flow',lineWidth:0}).trimEnd()}${eol}`});
      } else throw new Error('Cannot safely patch this YAML node');
    }
  }
  const comments=yamlComments(document.contents.srcToken);
  if (edits.some(edit=>comments.some(comment=>edit.start < comment.end && edit.end > comment.start))) {
    throw new Error('Cannot safely patch YAML without removing a comment; edit this field in Code');
  }
  const result=edits.sort((a,b)=>b.start-a.start).reduce((text,edit)=>text.slice(0,edit.start)+edit.text+text.slice(edit.end),source);
  const checked=parseDocument(result);
  if (checked.errors.length || !isDeepStrictEqual(checked.toJS(),{...previous,...fields})) throw new Error('Patch could not preserve YAML structure');
  return result;
}
/** fields is a partial top-level update; omitted body and fields retain original bytes. */
function patch(request, definition) {
  if (!request.fields || typeof request.fields !== 'object' || Array.isArray(request.fields)) throw new Error('fields must be an object');
  if (definition.kind === 'json') return {source:patchJson(request.source,request.fields,request.collection)};
  const match=/^(---\r?\n)([\s\S]*?)(^---(?:\r?\n|$))/m.exec(request.source);
  if (!match || match.index !== 0) throw new Error('Missing YAML frontmatter');
  const body=request.body === undefined ? request.source.slice(match[0].length) : request.body;
  if (typeof body !== 'string') throw new Error('body must be a string');
  return {source:match[1]+patchYaml(match[2],request.fields,request.collection)+match[3]+body};
}
export function handleRequest(request) {
  if (request.op === 'list') return {collections:list()};
  if (request.op === 'describe') return {collections:list().map(collection=>{
    const fields=schemas[collection.name] ? describeSchema(schemas[collection.name]) : {type:'unsupported',required:true,reason:'No shared schema'};
    return {...collection,fields,supportedForEditing:collection.supportedForEditing && !blocked(fields)};
  })};
  const schema=schemas[request.collection], definition=collectionRegistry[request.collection];
  if (!schema || !definition || !bundled.has(request.collection)) throw new Error('Unsupported collection');
  if (request.op === 'validate') {
    const parsed=schema.safeParse(request.fields);
    return parsed.success ? {valid:true,fields:parsed.data,errors:[]} : {valid:false,fields:request.fields,errors:parsed.error.issues.map(({path,message})=>({path,message}))};
  }
  if (request.op === 'patch') return patch(request,definition);
  throw new Error(`Unsupported operation: ${request.op}`);
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    for await (const line of createInterface({input:process.stdin,crlfDelay:Infinity})) if (line.trim()) process.stdout.write(JSON.stringify(handleRequest(JSON.parse(line)))+'\n');
  } catch (error) { process.stderr.write(`${error.message}\n`); process.exitCode=1; }
}
