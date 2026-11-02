/** Compare fact content across independent calls; observation times and their derived assessment IDs differ. */
export function comparable(value: unknown): unknown {
 return JSON.parse(JSON.stringify(value, (key, v) => ['observedAt','capturedAt','cutoff','checkedAt','assessmentId','baselineAssessmentId','baselineId'].includes(key) ? undefined : v));
}
