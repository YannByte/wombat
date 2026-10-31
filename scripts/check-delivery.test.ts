import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';
import { deliveryErrors } from './check-delivery.ts';

test('delivery rejects removed hosts, transport exports and extra Skills without build outputs', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-delivery-'));
  try {
    for (const name of ['client','cli','plugin']) mkdirSync(path.join(root,name));
    for (const file of ['package.json','client/package.json','cli/package.json']) writeFileSync(path.join(root,file),'{}');
    const metadata = path.join(root,'plugin/package.json');
    writeFileSync(metadata,JSON.stringify({skills:['wombat']}));
    assert.deepEqual(deliveryErrors(root),[]);
    mkdirSync(path.join(root,'web'));
    assert.match(deliveryErrors(root).join('\n'),/Removed delivery surface/);
    rmSync(path.join(root,'web'),{recursive:true});
    writeFileSync(path.join(root,'client/package.json'),JSON.stringify({exports:{'./http':'./http.js'}}));
    assert.match(deliveryErrors(root).join('\n'),/removed host dependency/);
    writeFileSync(path.join(root,'client/package.json'),'{}');
    mkdirSync(path.join(root,'plugin/hooks'));
    assert.match(deliveryErrors(root).join('\n'),/plugin\/hooks/);
    rmSync(path.join(root,'plugin/hooks'),{recursive:true});
    assert.deepEqual(deliveryErrors(root),[]);
    writeFileSync(metadata,JSON.stringify({skills:['wombat','another']}));
    assert.match(deliveryErrors(root).join('\n'),/exactly one/);
  } finally { rmSync(root,{recursive:true,force:true}); }
});
