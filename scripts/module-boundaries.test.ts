import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,writeFileSync,rmSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

test('runtime export is CLI-only and does not permit internal package imports', () => {
  const root=mkdtempSync(path.join(os.tmpdir(),'wombat-module-boundaries-'));
  try {
    for(const owner of ['client','cli','ui','web'])mkdirSync(path.join(root,owner,'src'),{recursive:true});
    const cli=path.join(root,'cli/src/index.ts'),ui=path.join(root,'ui/src/index.ts');
    const run=()=>spawnSync(process.execPath,[fileURLToPath(new URL('./check-module-boundaries.mjs',import.meta.url))],{cwd:root,encoding:'utf8',timeout:10000,maxBuffer:65536});
    writeFileSync(cli,"import {enableInstalledCompileCache} from '@wombat/client/node/runtime';");
    assert.equal(run().status,0);
    writeFileSync(ui,"import '@wombat/client/node/runtime';");
    assert.equal(run().status,1);
    rmSync(ui);
    writeFileSync(cli,"import '@wombat/client/src/node/runtime';");
    assert.equal(run().status,1);
  } finally {rmSync(root,{recursive:true,force:true});}
});
