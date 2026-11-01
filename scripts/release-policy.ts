import {readFileSync,readdirSync} from 'node:fs';
import path from 'node:path';

export function actionSources(root:string):Record<string,string> {
  const directory=path.join(root,'.github');
  return Object.fromEntries(readdirSync(directory,{recursive:true,encoding:'utf8'}).filter(f=>/\.ya?ml$/.test(f)).map(f=>[`.github/${f}`,readFileSync(path.join(directory,f),'utf8')]));
}
export interface CiRun {
  conclusion: string;
  event: string;
  headBranch: string;
  headSha: string;
  status: string;
  url: string;
}

export function repositorySlug(value: string): string {
  const match = value.trim().match(/(?:^|[/:@])github\.com[/:]([^/\s]+\/[^/\s]+?)(?:\.git)?$/);
  if (!match) throw new Error(`package.json repository is not a GitHub repository: ${value}`);
  return match[1];
}

export function successfulCiRun(runs: CiRun[], source: string, branch: string): CiRun | undefined {
  return runs.find(run => run.headSha === source && run.headBranch === branch && run.event === 'push'
    && run.status === 'completed' && run.conclusion === 'success');
}

export function rootReadmeReleaseErrors(
  readmes: { english: string; chinese: string },
  version: string,
  repository: string,
): string[] {
  const releaseUrl = `https://github.com/${repository}/releases/tag/v${version}`;
  const raw = `https://raw.githubusercontent.com/${repository}/main`;
  const stage = version.includes('-dev.') ? ['Development Preview:', '开发者预览版：']
    : version.includes('-beta.') ? ['Beta:', 'Beta 测试版：']
      : version.includes('-rc.') ? ['Release Candidate:', 'RC 候选版：']
        : ['Stable:', '正式版：'];
  const preview = version.includes('-');
  const installers = [
    `curl -fsSL ${raw}/scripts/install/install.sh | sh -s --${preview ? ` --version ${version}` : ''} --plugin`,
    `& ([scriptblock]::Create((irm ${raw}/scripts/install/install.ps1)))${preview ? ` -Version ${version}` : ''} -Plugin`,
  ];
  const requirements = [
    ['README.md', readmes.english, [
      `[\`v${version}\`](${releaseUrl})`,
      ...installers, '## Update', '(docs/guides/installation.en.md)',
      'macOS arm64/x64', 'Linux glibc arm64/x64', 'Windows x64',
    ]],
    ['README.zh-CN.md', readmes.chinese, [
      `[\`v${version}\`](${releaseUrl})`,
      ...installers, '## 更新', '(docs/guides/installation.md)',
      'macOS arm64/x64', 'Linux glibc arm64/x64', 'Windows x64',
    ]],
  ] as const;
  const errors: string[] = [];
  for (const [file, content, expected] of requirements) {
    for (const text of expected) if (!content.includes(text)) errors.push(`${file}: missing current release fact: ${text}`);
  }
  if (!readmes.english.includes(stage[0])) errors.push(`README.md: missing ${stage[0].slice(0, -1)} stage`);
  if (!readmes.chinese.includes(stage[1])) errors.push(`README.zh-CN.md: missing ${stage[1].slice(0, -1)} stage`);
  for (const [file, content] of [['README.md', readmes.english], ['README.zh-CN.md', readmes.chinese]] as const) {
    for (const stale of [
      'becomes available after the release workflow completes',
      '发行工作流完成前，下面的版本化安装地址暂不可用',
      'npm install -g wombat',
    ]) if (content.includes(stale)) errors.push(`${file}: stale release text: ${stale}`);
  }
  if (version !== '0.1.0-dev.2') {
    if (readmes.english.includes('first public preview')) errors.push('README.md: stale first-release description');
    if (readmes.chinese.includes('首个公开预览版本')) errors.push('README.zh-CN.md: stale first-release description');
  }
  return errors;
}

export function actionPinErrors(workflows: Record<string, string>): string[] {
  const errors: string[] = [];
  for (const [file, content] of Object.entries(workflows)) {
    for (const [index, line] of content.split('\n').entries()) {
      const match = line.match(/^\s*(?:-\s*)?uses:\s*([^\s#]+)/);
      if (!match || match[1].startsWith('./')) continue;
      if (match[1].startsWith('docker://')) {
        if (!/@sha256:[0-9a-f]{64}$/.test(match[1])) {
          errors.push(`${file}:${index + 1}: external container Action must use a SHA-256 digest: ${match[1]}`);
        }
        continue;
      }
      const separator = match[1].lastIndexOf('@');
      const revision = separator >= 0 ? match[1].slice(separator + 1) : '';
      if (!/^[0-9a-f]{40}$/.test(revision)) {
        errors.push(`${file}:${index + 1}: external Action must use a full commit SHA: ${match[1]}`);
      }
    }
  }
  return errors;
}
