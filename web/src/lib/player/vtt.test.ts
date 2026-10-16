import { describe, expect, it } from 'vitest';
import { cueAt, parseVtt } from './vtt';

describe('vtt', () => {
  it('parses and finds cues', () => {
    const cues = parseVtt('WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.500\n<i>Привет</i>\n\n00:01:00.000 --> 00:01:02.000 align:start\n{\\an8}Мир\n');
    expect(cues).toHaveLength(2);
    expect(cueAt(cues, 1.5)).toBe('Привет');
    expect(cueAt(cues, 3)).toBe('');
    expect(cueAt(cues, 61)).toBe('Мир');
  });
});
