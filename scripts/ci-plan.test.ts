import assert from 'node:assert/strict';
import test from 'node:test';
import {documentationOnly,gateResults,heavyChecks} from './ci-plan.ts';
test('documentation filter excludes prose only and fails open for product inputs',()=>{
  assert.equal(documentationOnly(['README.md','docs/guides/cli.en.md','docs/guides/cli.i18n.json']),true);
  assert.equal(documentationOnly(['client/src/messages.i18n.json']),false);
  for(const files of [[],['AGENTS.md'],['docs/AGENTS.md'],['plugin/skills/wombat/SKILL.md'],['.agents/skills/wombat-verify/SKILL.md'],['docs/schemas/request.json'],['docs/dependency-licenses.json'],['README.md','core/src/lib.rs'],['.github/workflows/ci.yml'],['package.json']])assert.equal(documentationOnly(files),false,files.join(','));
});
test('every main push and explicit dispatch runs all checks; only confirmed PR prose can skip',()=>{
  assert.equal(heavyChecks('push','refs/heads/main',['README.md']),true);
  assert.equal(heavyChecks('workflow_dispatch','refs/heads/main',['README.md']),true);
  assert.equal(heavyChecks('pull_request','refs/pull/1/merge',['README.md']),false);
  assert.equal(heavyChecks('pull_request','refs/pull/1/merge',[]),true);
  assert.equal(heavyChecks('pull_request','refs/pull/1/merge',['README.md','core/src/lib.rs']),true);
});
test('stable required gate accepts complete or explicitly skipped docs checks only',()=>{
  assert.equal(gateResults(true,{a:{result:'success'},b:{result:'success'}}),true);
  assert.equal(gateResults(false,{a:{result:'skipped'}}),true);
  for(const result of ['failure','cancelled','skipped'])assert.equal(gateResults(true,{a:{result}}),false);
  for(const result of ['failure','cancelled','success'])assert.equal(gateResults(false,{a:{result}}),false);
  assert.equal(gateResults(true,{}),false);
});
