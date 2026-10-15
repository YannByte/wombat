import {copyFile,readdir} from 'node:fs/promises';
// Rust generation owns the validator inventory. New methods must reach every host.
for (const entry of await readdir(new URL('../src/generated/',import.meta.url),{withFileTypes:true})) {
  if(entry.isFile() && /^(?:validate-[a-z0-9-]+|schema-agent-(?:request|results))\.(?:js|d\.ts)$/.test(entry.name)) {
    await copyFile(new URL('../src/generated/'+entry.name,import.meta.url),new URL('../dist/generated/'+entry.name,import.meta.url));
  }
}
