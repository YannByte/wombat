import { copyFile } from 'node:fs/promises';
for (const name of ['conversation-request','conversation-response','monitor-request','monitor-response','agent-error','agent-request','agent-description','setup-request','setup-response','collection-request','collection-response','web-view-request','skill-installation','skill-discovery','timing-error-output','timing-request','timing-response','timing-local-response','timing-share-response','handoff-request','handoff-response','account-request','account-response','directories-request','directories-response','analysis-declaration', 'preferences-request', 'preferences-response', 'optimize-request', 'optimize-response', 'config-request', 'config-response', 'live-request', 'live-response', 'usage-request', 'usage-app', 'pricing-request', 'pricing-response']) {
  for (const extension of ['js', 'd.ts']) {
    await copyFile(new URL(`../src/generated/validate-${name}.${extension}`, import.meta.url), new URL(`../dist/generated/validate-${name}.${extension}`, import.meta.url));
  }
}

for(const extension of ['js','d.ts'])await copyFile(new URL(`../src/generated/schema-agent-request.${extension}`,import.meta.url),new URL(`../dist/generated/schema-agent-request.${extension}`,import.meta.url));

for(const extension of ['js','d.ts'])await copyFile(new URL(`../src/generated/schema-agent-results.${extension}`,import.meta.url),new URL(`../dist/generated/schema-agent-results.${extension}`,import.meta.url));
