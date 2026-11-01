import { execFileSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import {nativeTargets} from './native-platforms.ts';

export const requiredCandidateJobs = ['Repository rules before build', 'Dependency advisories',
  'CI', 'Rust formatting', 'Source-only checks on macos-15', 'Source-only checks on windows-2025',
  'Assemble GitHub Release candidate', ...nativeTargets.flatMap(target => [`Core tests ${target}`, `Verify ${target}`, `Install archive on ${target}`])];

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function validateCiCandidate(run: unknown, listing: unknown, jobs: unknown, repository: string, source: string, branch: string, now = Date.now()): {runId: number; artifactId: number; url: string} {
  if (!record(run) || !Number.isSafeInteger(run.id) || Number(run.id) <= 0 || run.head_sha !== source
    || !Number.isSafeInteger(run.run_attempt) || Number(run.run_attempt) <= 0
    || run.head_branch !== branch || run.event !== 'push' || run.status !== 'completed' || run.conclusion !== 'success'
    || run.path !== '.github/workflows/ci.yml' || !record(run.repository) || run.repository.full_name !== repository
    || typeof run.html_url !== 'string') throw new Error('Candidate requires successful push CI for the exact repository, source and branch');
  if (!record(jobs) || !Array.isArray(jobs.jobs) || jobs.total_count !== jobs.jobs.length) throw new Error('CI job evidence is incomplete');
  for (const name of requiredCandidateJobs) {
    const matches = jobs.jobs.filter(job => record(job) && job.name === name);
    if (matches.length !== 1 || !record(matches[0]) || matches[0].status !== 'completed' || matches[0].conclusion !== 'success'
      || matches[0].run_attempt !== run.run_attempt) {
      throw new Error(`Required candidate acceptance did not pass: ${name}`);
    }
  }
  if (!record(listing) || !Array.isArray(listing.artifacts) || !Number.isSafeInteger(listing.total_count)
    || listing.total_count !== listing.artifacts.length) throw new Error('CI artifact listing is incomplete');
  const matches = listing.artifacts.filter(asset => record(asset) && asset.name === 'github-candidate');
  if (matches.length !== 1) throw new Error('CI must contain exactly one github-candidate artifact; inspect candidate assembly');
  const asset = matches[0];
  if (!record(asset) || !Number.isSafeInteger(asset.id) || Number(asset.id) <= 0 || asset.expired !== false
    || typeof asset.expires_at !== 'string' || !Number.isFinite(Date.parse(asset.expires_at)) || Date.parse(asset.expires_at) <= now
    || !record(asset.workflow_run) || asset.workflow_run.id !== run.id || asset.workflow_run.head_sha !== source
    || typeof asset.digest !== 'string' || !/^sha256:[0-9a-f]{64}$/.test(asset.digest)) {
    throw new Error('CI candidate is expired or has a different identity; renew exact-source CI before tagging, then resume release:publish');
  }
  return {runId: Number(run.id), artifactId: Number(asset.id), url: run.html_url};
}

export function readCiCandidate(repository: string, runId: number, source: string, branch: string) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository) || !Number.isSafeInteger(runId) || runId <= 0) throw new Error('Invalid CI identity');
  const read = (endpoint: string): unknown => JSON.parse(execFileSync('gh', ['api', endpoint], {encoding: 'utf8', timeout: 60_000, maxBuffer: 2 * 1024 * 1024}));
  return validateCiCandidate(read(`repos/${repository}/actions/runs/${runId}`),
    read(`repos/${repository}/actions/runs/${runId}/artifacts?per_page=100`),
    read(`repos/${repository}/actions/runs/${runId}/jobs?filter=latest&per_page=100`), repository, source, branch);
}

const entry = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : '';
if (entry === import.meta.url) {
  try {
    const {values} = parseArgs({args: process.argv.slice(2), options: {run: {type: 'string'}, output: {type: 'string'}}});
    if (!values.run || !values.output || !process.env.GITHUB_REPOSITORY || !process.env.GITHUB_SHA || !process.env.DEFAULT_BRANCH) throw new Error('CI candidate verification requires run, output and GitHub source environment');
    const candidate = readCiCandidate(process.env.GITHUB_REPOSITORY, Number(values.run), process.env.GITHUB_SHA, process.env.DEFAULT_BRANCH);
    appendFileSync(values.output, `ci_run_id=${candidate.runId}\nartifact_id=${candidate.artifactId}\n`);
    console.log(`Reusing verified CI candidate: ${candidate.url}`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}
