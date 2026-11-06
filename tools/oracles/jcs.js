// Independent oracle for crates/wanjiru-docir: ECMAScript is the reference implementation of the
// number formatting and the string escaping that RFC 8785 (JSON Canonicalization Scheme) prescribes,
// and the default sort of JavaScript strings is by UTF-16 code unit, the order the RFC requires.
//
// Usage: node jcs.js < request.json > response.json
//   request:  {"numbers": ["<16 hex digits of an IEEE-754 double>", ...], "texts": ["<hex of UTF-8 JSON text>", ...]}
//   response: {"numbers": ["<Number::toString>", ...], "canonical": ["<hex of canonical UTF-8>" | null, ...]}
// A text that JSON.parse cannot read, or that contains a lone surrogate, gives null.
"use strict";
const fs = require("fs");

const request = JSON.parse(fs.readFileSync(0, "utf8"));

const view = new DataView(new ArrayBuffer(8));
const numbers = request.numbers.map((hex) => {
  view.setBigUint64(0, BigInt("0x" + hex));
  return String(view.getFloat64(0));
});

function hasLoneSurrogate(text) {
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    if (c >= 0xd800 && c <= 0xdbff) {
      const d = text.charCodeAt(i + 1);
      if (!(d >= 0xdc00 && d <= 0xdfff)) return true;
      i++;
    } else if (c >= 0xdc00 && c <= 0xdfff) {
      return true;
    }
  }
  return false;
}

function canonicalize(value) {
  if (value === null || typeof value === "boolean" || typeof value === "number") {
    return JSON.stringify(value);
  }
  if (typeof value === "string") {
    if (hasLoneSurrogate(value)) throw new Error("lone surrogate");
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return "[" + value.map(canonicalize).join(",") + "]";
  }
  const keys = Object.keys(value).sort(); // by UTF-16 code units
  return (
    "{" +
    keys
      .map((key) => {
        if (hasLoneSurrogate(key)) throw new Error("lone surrogate");
        return JSON.stringify(key) + ":" + canonicalize(value[key]);
      })
      .join(",") +
    "}"
  );
}

const canonical = request.texts.map((hex) => {
  try {
    const text = Buffer.from(hex, "hex").toString("utf8");
    return Buffer.from(canonicalize(JSON.parse(text)), "utf8").toString("hex");
  } catch (error) {
    return null;
  }
});

process.stdout.write(JSON.stringify({ numbers, canonical }));
