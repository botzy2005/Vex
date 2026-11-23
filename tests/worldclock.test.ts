import { describe, expect, it } from 'vitest';
import { CITIES } from '../src/hub/widgets/worldclock/cities';
import {
  addClock, DEFAULT_DOC, dayLabel, diffLabel, hourIn, isDaytime, isValidZone, MAX_CLOCKS, moveClock, msToNextMinute, normalize,
  offsetMinutes, removeClock, searchCities, timeIn, type ClockDoc,
} from '../src/hub/widgets/worldclock/logic';

const at = (y: number, mo: number, d: number, h = 12, mi = 0) => Date.UTC(y, mo - 1, d, h, mi);

describe('time zones', () => {
  it('knows fixed offsets', () => {
    expect(offsetMinutes('Asia/Kolkata', at(2026, 11, 23))).toBe(330);
    expect(offsetMinutes('Asia/Kathmandu', at(2026, 11, 23))).toBe(345);
    expect(offsetMinutes('Asia/Tokyo', at(2026, 7, 1))).toBe(540);
    expect(offsetMinutes('UTC', at(2026, 7, 1))).toBe(0);
  });
  it('follows daylight saving in both hemispheres', () => {
    expect(offsetMinutes('America/New_York', at(2026, 1, 15))).toBe(-300);
    expect(offsetMinutes('America/New_York', at(2026, 7, 15))).toBe(-240);
    expect(offsetMinutes('Europe/London', at(2026, 7, 15))).toBe(60);
    expect(offsetMinutes('Europe/London', at(2026, 12, 15))).toBe(0);
    expect(offsetMinutes('Australia/Sydney', at(2026, 1, 15))).toBe(660);
    expect(offsetMinutes('Australia/Sydney', at(2026, 7, 15))).toBe(600);
  });
  it('changes offset across the exact switch instant', () => {
    // US spring-forward 2026-03-08 07:00 UTC
    expect(offsetMinutes('America/New_York', at(2026, 3, 8, 6, 59))).toBe(-300);
    expect(offsetMinutes('America/New_York', at(2026, 3, 8, 7, 0))).toBe(-240);
  });
  it('every listed city has a valid, unique zone', () => {
    const zones = CITIES.map((c) => c.zone);
    expect(new Set(zones).size).toBe(zones.length);
    for (const z of zones) expect(isValidZone(z), z).toBe(true);
  });
  it('rejects bad zone ids', () => {
    for (const z of ['', 'Mars/Olympus', 'x'.repeat(80), '../etc']) expect(isValidZone(z)).toBe(false);
  });
});

describe('formatting', () => {
  it('shows the wall-clock time in 24 and 12 hour styles', () => {
    expect(timeIn('Asia/Kolkata', at(2026, 11, 23, 12, 0), false)).toBe('17:30');
    expect(timeIn('Asia/Kolkata', at(2026, 11, 23, 12, 0), true)).toMatch(/^5:30\s?pm$/i);
    expect(timeIn('America/New_York', at(2026, 11, 23, 12, 5), false)).toBe('07:05');
  });
  it('labels the day relative to the local zone', () => {
    const t = at(2026, 11, 23, 22, 0); // 03:30 on the 24th in Kolkata, 17:00 on the 23rd in New York
    expect(dayLabel('Asia/Kolkata', 'America/New_York', t)).toBe('Tomorrow');
    expect(dayLabel('America/New_York', 'America/New_York', t)).toBe('Today');
    expect(dayLabel('America/New_York', 'Asia/Kolkata', t)).toBe('Yesterday');
    expect(dayLabel('Pacific/Kiritimati', 'Pacific/Pago_Pago', at(2026, 11, 23, 10, 30))).toMatch(/\d/); // two calendar days apart: a date, not a word
  });
  it('describes the difference to the local zone', () => {
    const t = at(2026, 11, 23);
    expect(diffLabel('Asia/Kolkata', 'Asia/Kolkata', t)).toBe('same time');
    expect(diffLabel('Asia/Kolkata', 'UTC', t)).toBe('+5h 30m');
    expect(diffLabel('Europe/London', 'Asia/Kolkata', t)).toBe('−5h 30m');
    expect(diffLabel('Asia/Kathmandu', 'Asia/Kolkata', t)).toBe('+15m');
    expect(diffLabel('Asia/Tokyo', 'UTC', t)).toBe('+9h');
  });
  it('tells day from night', () => {
    expect(hourIn('Asia/Kolkata', at(2026, 11, 23, 0, 0))).toBe(5);
    expect(isDaytime('Asia/Kolkata', at(2026, 11, 23, 0, 29))).toBe(false); // 05:59
    expect(isDaytime('Asia/Kolkata', at(2026, 11, 23, 0, 30))).toBe(true); // 06:00
    expect(isDaytime('Asia/Kolkata', at(2026, 11, 23, 12, 29))).toBe(true); // 17:59
    expect(isDaytime('Asia/Kolkata', at(2026, 11, 23, 12, 30))).toBe(false); // 18:00
  });
  it('wakes at the next whole minute', () => {
    expect(msToNextMinute(60000 * 5)).toBe(60020);
    expect(msToNextMinute(60000 * 5 + 59_990)).toBe(30);
  });
});

describe('clock list', () => {
  it('survives garbage and falls back to the starter list', () => {
    expect(normalize(null)).toEqual(DEFAULT_DOC);
    expect(normalize({ clocks: 'x' })).toEqual(DEFAULT_DOC);
    const d = normalize({ hour12: 'yes', clocks: [{ zone: 'Asia/Tokyo' }, { zone: 'Asia/Tokyo', label: 'dup' }, { zone: 'Mars/X' }, null, { zone: 'UTC', label: '  Zulu  ' }] });
    expect(d.clocks).toEqual([{ zone: 'Asia/Tokyo', label: 'Tokyo' }, { zone: 'UTC', label: 'Zulu' }]);
    expect(d.hour12).toBeNull();
    expect(normalize({ clocks: [] }).clocks).toEqual([]); // an empty list the user chose stays empty
  });
  it('caps the number of clocks and the label length', () => {
    const clocks = CITIES.slice(0, 30).map((c) => ({ zone: c.zone, label: c.name }));
    expect(normalize({ clocks }).clocks).toHaveLength(MAX_CLOCKS);
    expect(normalize({ clocks: [{ zone: 'UTC', label: 'x'.repeat(100) }] }).clocks[0].label).toHaveLength(40);
  });
  it('adds without duplicates or going past the limit, removes and reorders', () => {
    let d: ClockDoc = { version: 1, hour12: null, clocks: [] };
    const tokyo = CITIES.find((c) => c.name === 'Tokyo')!;
    d = addClock(d, tokyo);
    expect(addClock(d, tokyo)).toBe(d);
    d = addClock(d, CITIES.find((c) => c.name === 'Paris')!);
    d = moveClock(d, 'Europe/Paris', -1);
    expect(d.clocks.map((c) => c.label)).toEqual(['Paris', 'Tokyo']);
    expect(moveClock(d, 'Europe/Paris', -1)).toBe(d);
    expect(moveClock(d, 'Asia/Tokyo', 1)).toBe(d);
    expect(removeClock(d, 'Asia/Tokyo').clocks).toHaveLength(1);
    expect(removeClock(d, 'nope')).toBe(d);
    let full = d;
    for (const c of CITIES) full = addClock(full, c);
    expect(full.clocks).toHaveLength(MAX_CLOCKS);
  });
  it('searches by city, country or zone, skipping added ones', () => {
    expect(searchCities('tok').map((c) => c.name)).toEqual(['Tokyo']);
    expect(searchCities('india').map((c) => c.name)).toEqual(['Kolkata']);
    expect(searchCities('buenos').map((c) => c.name)).toEqual(['Buenos Aires']);
    expect(searchCities('america new').map((c) => c.name)).toContain('New York');
    expect(searchCities('tokyo', ['Asia/Tokyo'])).toEqual([]);
    expect(searchCities('   ')).toEqual([]);
    expect(searchCities('a')).toHaveLength(8);
  });
});
