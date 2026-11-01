import assert from 'node:assert/strict';
import test from 'node:test';
import {runSystemCli} from '../src/system-cli.js';

test('invalid host operations fail before probing or starting any runtime',async()=>{
  for(const args of [['missing'],['doctor','--plugin','--plugin'],['watch','--interval','4'],['watch','--interval','3601'],['watch','--interval','1.5'],['watch','--root'],['watch','--id='],['watch','--once=x'],['watch','--json=x'],['watch','--json','--json']]) {
    await assert.rejects(runSystemCli(args),{code:'INVALID_ARGUMENT'},args.join(' '));
  }
});
