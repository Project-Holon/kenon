// Which eidos launch is the latest: the move preview or Test in game. Both
// report a refusal through the same `preview-ended` event (one eidos at a
// time, app run_eidos), so each screen shows only its own.
export const lastRun: { kind: "preview" | "test" | null } = { kind: null };
