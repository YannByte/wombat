/** Presentation only: use the published subset and ratio, never derive outcome facts. */
import type {TimingLocalResult} from '../index.js';
import {locale,t,type MessageKey} from './index.js';
export function operationOutcomeText(o:TimingLocalResult['work']['outcomes']):{headline:string;details:string[];note:string}{
 const headline=o.failureRatio.value!=null&&o.determinateOperations.value!=null&&o.failed.value!=null
  ?t('execution.outcomes.ratio',{failed:o.failed.value,total:o.determinateOperations.value,ratio:new Intl.NumberFormat(locale.getLocale(),{style:'percent',maximumFractionDigits:1}).format(o.failureRatio.value)})
  :o.determinateOperations.value===0?t('execution.outcomes.empty'):t('execution.outcomes.unavailable');
 const rows:Array<[Extract<MessageKey,'execution.outcomes.interrupted'|'execution.outcomes.rejected'|'execution.outcomes.nonterminal'|'execution.outcomes.indeterminate'|'execution.outcomes.conflicting'|'execution.outcomes.identity'|'execution.outcomes.unclassified'>,TimingLocalResult['work']['outcomes']['failed']]>=[
  ['execution.outcomes.interrupted',o.interrupted],['execution.outcomes.rejected',o.rejected],['execution.outcomes.nonterminal',o.nonterminal],
  ['execution.outcomes.indeterminate',o.indeterminate],['execution.outcomes.conflicting',o.conflicting],['execution.outcomes.identity',o.identityGapRecords],['execution.outcomes.unclassified',o.unclassified]];
 const details=rows.flatMap(([key,value])=>value.value!=null&&value.value>0?[t(key,{count:value.value})]:[]);
 if(o.partial)details.push(t('execution.outcomes.partial'));
 return {headline,details,note:t('execution.outcomes.note')};
}
