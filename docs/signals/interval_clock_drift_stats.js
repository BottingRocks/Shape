// IntervalClockDriftStats. Captured example values.
export default {
  "maxTickDeltaMs": 1838,
  "meanTickDeltaMs": 909.7,
  "minTickDeltaMs": 185,
  "tickCount": 49
};

// Illustrative startup placeholder, before any timer tick.
export const startup = {
  "meanTickDeltaMs": -1,
  "minTickDeltaMs": -1,
  "maxTickDeltaMs": -1,
  "tickCount": 0
};

// Illustrative: the system clock jumped backwards between ticks, so the smallest gap is negative.
export const clockJumpedBackwards = {
  "meanTickDeltaMs": 997.4,
  "minTickDeltaMs": -1846,
  "maxTickDeltaMs": 1013,
  "tickCount": 34
};

// Illustrative: a background tab throttled the timer, so the gaps are about a minute.
export const backgroundThrottled = {
  "meanTickDeltaMs": 60028.6,
  "minTickDeltaMs": 59714,
  "maxTickDeltaMs": 60381,
  "tickCount": 9
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "maxTickDeltaMs": 2018,
  "meanTickDeltaMs": 1300.1,
  "minTickDeltaMs": 970,
  "tickCount": 21
};
