// Development-only extraction of unchanged encoder bodies. Run from any cwd.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
if (process.argv.length !== 4) {
 console.error('Usage: node superpack-extract.cjs <source> <alternate-source>');
 process.exit(1);
}
const [sourcePath, alternateSourcePath] = process.argv.slice(2);
const source = fs.readFileSync(sourcePath, 'utf8');
const sha256 = crypto.createHash('sha256').update(source).digest('hex');
if (sha256 !== 'f95e7e6049cfa4f6cf07f59385eb0d02239b905f90d4b592afed58498078b6c6') throw Error('Source changed: re-audit ranges');
// Validate both source identities before writing either reference or provenance file.
const alternateSource = fs.readFileSync(alternateSourcePath, 'utf8');
const alternateHash = crypto.createHash('sha256').update(alternateSource).digest('hex');
if (alternateHash !== '472fd0f679bd46ceae81714817cf8ea0e42bfecdd08d3a196e758f6c972b8683') throw Error('Alternate encoder source changed: re-audit ranges');
const spans = [[2567,2574],[2611,2639],[14363,14584],[14868,15290]];
const lines = source.split('\n');
const prelude = `'use strict';\nlet result8 = Object.create(null); // source identity sentinel line 4569\nlet encodingTags, handler2, j, value53, encode2;\n`;
const glue = `
function packValue(value, options) { return encode2(value, options); }
function signalOptions(keysetsToOmit = [], depthBound = 100) {
 return {keysetsToOmit, depthBound, extensions: {0: value53, 3735928559: j3}};
}
module.exports = {packValue, signalOptions, missingSentinel:result8};
`;
const output = `// Exact source extraction; source SHA-256 ${sha256}\n` + prelude +
 spans.map(([a,b])=>`\n// SOURCE ${a}-${b}\n`+lines.slice(a-1,b).join('\n')).join('\n') + glue;
fs.writeFileSync(path.join(__dirname,'superpack-reference.cjs'), output);
fs.writeFileSync(path.join(__dirname,'superpack-provenance.json'), JSON.stringify({source:`sha256:${sha256}`,sha256,spans,
 reference_file:'superpack-reference.cjs',reference_sha256:crypto.createHash('sha256').update(output).digest('hex'),
 changes:['No body edits or identifier renames. Only lexical prelude, identity sentinel, module exports, and option-construction glue.',
 'Typed-array float path on little-endian Node. Fallback float path retained but not claimed compatible.',
 'Original source encoder only; no browser/signal collection, measurements, private headers or captured payloads.']},null,2)+'\n');
// Independent alternate encoder, copied without body changes.
const alternateSpans = [[128,133],[2491,2519],[2723,2731],[13718,13720],[13723,13947],[13948,14367]];
const alternateOutput = `// Exact source extraction; SHA-256 ${alternateHash}\n'use strict';\nlet j, prototype; let result13=Object.create(null);\n` +
 alternateSpans.map(([a,b])=>`\n// SOURCE ${a}-${b}\n`+alternateSource.split('\n').slice(a-1,b).join('\n')).join('\n') + `
function signalOptions(keysetsToOmit=[],depthBound=100) {
 return {keysetsToOmit,depthBound,extensions:{0:prototype3,3735928559:prototype2}};
}
module.exports = {packValue:encode, signalOptions, missingSentinel:result13};
`;
fs.writeFileSync(path.join(__dirname,'superpack-reference-alternate.cjs'),alternateOutput);
fs.writeFileSync(path.join(__dirname,'superpack-provenance-alternate.json'),JSON.stringify({source:`sha256:${alternateHash}`,sha256:alternateHash,spans:alternateSpans,
 reference_file:'superpack-reference-alternate.cjs',reference_sha256:crypto.createHash('sha256').update(alternateOutput).digest('hex'),
 changes:['Unchanged original bodies; lexical prelude, identity sentinel at source line 4393, exports and option glue only.']},null,2)+'\n');
