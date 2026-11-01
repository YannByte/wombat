import type {CliHelp} from '@wombat/client';
import {t} from '@wombat/client/locale';
import {commandNames} from './command-routing.js';

type Topic='root'|'history'|'usage'|'check'|'clean'|'system'|'alerts';
const helpKeys={root:'cli.usage-app-cli.help',history:'cli.history.help',usage:'cli.usage.help',check:'cli.project.help',clean:'artifacts.help',system:'cli.system.help',alerts:'cli.alerts.help'} as const;
const examples={
  root:[['history','wombat history search "release"'],['usage','wombat usage week'],['check','wombat check'],['clean','wombat clean'],['system','wombat system doctor']],
  history:[['history','wombat history search "release"'],['read','wombat history read RECORD --find "validation"'],['details','wombat history show RECORD']],
  usage:[['usage','wombat usage week'],['top','wombat usage top week'],['compare','wombat usage compare'],['stats','wombat usage stats']],
  check:[['check','wombat check --project PATH'],['skills','wombat check skills --project PATH'],['task','wombat check --task RECORD --turn TURN'],['verify','wombat check verify --read-view VIEW --baseline BASELINE --project PATH']],
  clean:[['clean','wombat clean --project PATH'],['codex','wombat clean --codex'],['apply','wombat clean --codex --apply'],['verify','wombat clean verify --read-view VIEW --baseline BASELINE']],
  system:[['system','wombat system doctor'],['update','wombat system update --check'],['api','wombat system api METHOD --json']],
  alerts:[['alerts','wombat usage alerts --once'],['watch','wombat usage alerts --id PLAN_ID']],
} as const;
const taskKeys={history:'cli.help.history',usage:'cli.help.usage',check:'cli.help.check',clean:'cli.help.clean',system:'cli.help.system',read:'cli.help.read',details:'cli.help.details',top:'cli.help.top',compare:'cli.help.compare',stats:'cli.help.stats',skills:'cli.help.skills',task:'cli.help.task',verify:'cli.help.verify',codex:'cli.help.codex',apply:'cli.help.apply',update:'cli.help.update',api:'cli.help.api',alerts:'cli.help.alerts',watch:'cli.help.watch'} as const;
const methods={history:'conversation',usage:'usage',check:'project',clean:'artifacts',system:undefined,alerts:'monitor',root:undefined} as const;

/** Task selection fits one local read; full generated schemas remain opt-in per method. */
export function commandHelp(topic:Topic,args:readonly string[]):CliHelp {
  const action=topic==='usage'?args[1]:args[0];
  const selected=action&&!action.startsWith('-')?examples[topic].filter(([,command])=>command.startsWith('wombat '+topic+' '+action+' ')||command==='wombat '+topic+' '+action):[];
  const choices=selected.length?selected:examples[topic];
  return {outputVersion:1,command:topic==='root'?'wombat':topic==='alerts'?'wombat usage alerts':'wombat '+topic,commands:topic==='root'?commandNames:[],help:t(helpKeys[topic]),examples:choices.map(([task,command])=>({task:t(taskKeys[task]),command})),next:topic==='root'?commandNames.map(c=>`wombat ${c} --help --json`):methods[topic]?[`wombat system api ${methods[topic]} --json`]:['wombat system api --json']};
}
export function writeHelp(topic:Topic,args:readonly string[]):void {
  const help=commandHelp(topic,args);
  process.stdout.write(args.includes('--json')?JSON.stringify(help)+'\n':help.help);
}
