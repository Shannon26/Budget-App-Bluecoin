import { expect, test } from "vitest";
import { formatMinor, parseMinorUnits } from "./money";

test("parses rupee amounts into integer paise", () => {
  expect(parseMinorUnits("1000")).toBe(100000);
  expect(parseMinorUnits("250.50")).toBe(25050);
  expect(parseMinorUnits("1,000.05")).toBe(100005);
});

test("parses valid Western and Indian digit grouping", () => {
  expect(parseMinorUnits("1,000")).toBe(100000);
  expect(parseMinorUnits("1,000,000")).toBe(100000000);
  expect(parseMinorUnits("1,23,456")).toBe(12345600);
  expect(parseMinorUnits("12,34,567.89")).toBe(123456789);
});

test("rejects malformed comma placement instead of silently changing the amount", () => {
  expect(parseMinorUnits("12,50")).toBeNull();
  expect(parseMinorUnits("1,,000")).toBeNull();
  expect(parseMinorUnits("12,34")).toBeNull();
  expect(parseMinorUnits("1234,567")).toBeNull();
});

test("rejects fractional paise and negative transaction amounts", () => {
  expect(parseMinorUnits("1.001")).toBeNull();
  expect(parseMinorUnits("-10")).toBeNull();
  expect(parseMinorUnits("")).toBeNull();
});

test("allows negative opening balances when requested", () => {
  expect(parseMinorUnits("-25.50", true)).toBe(-2550);
});

test("rejects amounts outside JavaScript's safe integer range", () => {
  expect(parseMinorUnits("90071992547409.92")).toBeNull();
});

test("formats minor units for the browser", () => {
  expect(formatMinor(75000)).toContain("750.00");
  expect(formatMinor(45000)).toContain("450.00");
});
