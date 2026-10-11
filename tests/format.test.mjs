import { test } from 'node:test';
import assert from 'node:assert/strict';
import { formatCountdown, formatFocusTime } from '../src/format.ts';
test('focus duration carries minutes into hours', () => {
  for (const [minutes, text] of [[0, '0 分钟'], [59, '59 分钟'], [60, '1 小时'], [61, '1 小时 1 分钟'], [120, '2 小时'], [150, '2 小时 30 分钟']]) assert.equal(formatFocusTime(minutes), text);
});
test('countdown crosses the hour boundary and keeps seconds', () => {
  for (const [seconds, text] of [[0, '00:00'], [1, '00:01'], [3599, '59:59'], [3600, '1:00:00'], [3661, '1:01:01'], [10800, '3:00:00']]) assert.equal(formatCountdown(seconds), text);
});
