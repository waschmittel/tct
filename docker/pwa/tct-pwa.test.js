// Run with: node --test docker/pwa/tct-pwa.test.js
// Pins the PDF transfer format against src/export/web.rs.
'use strict';
const test = require('node:test');
const assert = require('node:assert');
const { createPdfReceiver, safePdfName } = require('./tct-pwa.js');

// Exactly what `send(out, "a b.pdf", b"%PDF-1.7\n")` writes in Rust, without ESC ] 7733 ; and BEL.
const FIXTURE = '0;1;YSBiLnBkZg==;JVBERi0xLjcK';

function b64(bytes) {
  return Buffer.from(bytes).toString('base64');
}

test('accepts the sequence produced by tct', () => {
  const pdf = createPdfReceiver().accept(FIXTURE);
  assert.strictEqual(pdf.name, 'a b.pdf');
  assert.strictEqual(Buffer.from(pdf.bytes).toString(), '%PDF-1.7\n');
});

test('reassembles chunks in order', () => {
  const receiver = createPdfReceiver();
  const name = b64('big.pdf');
  const body = Buffer.concat([Buffer.from('%PDF-1.7\n'), Buffer.alloc(50000, 7)]);
  const parts = [body.subarray(0, 24576), body.subarray(24576, 49152), body.subarray(49152)];
  assert.strictEqual(receiver.accept(`0;3;${name};${b64(parts[0])}`), null);
  assert.strictEqual(receiver.accept(`1;3;${name};${b64(parts[1])}`), null);
  const pdf = receiver.accept(`2;3;${name};${b64(parts[2])}`);
  assert.deepStrictEqual(Buffer.from(pdf.bytes), body);
});

test('drops out-of-order, malformed and non-PDF data', () => {
  const name = b64('x.pdf');
  assert.strictEqual(createPdfReceiver().accept(`1;2;${name};${b64('%PDF-')}`), null, 'missing first chunk');
  assert.strictEqual(createPdfReceiver().accept('garbage'), null);
  assert.strictEqual(createPdfReceiver().accept(`0;1;${name};!!!`), null);
  assert.strictEqual(createPdfReceiver().accept(`0;1;${name};${b64('<html>')}`), null, 'not a PDF');
  assert.strictEqual(createPdfReceiver().accept(`5;2;${name};${b64('%PDF-')}`), null, 'index beyond total');
});

test('a new transfer replaces an unfinished one', () => {
  const receiver = createPdfReceiver();
  const name = b64('x.pdf');
  receiver.accept(`0;2;${name};${b64('%PDF-aaaa')}`);
  const pdf = receiver.accept(`0;1;${name};${b64('%PDF-bbbb')}`);
  assert.strictEqual(Buffer.from(pdf.bytes).toString(), '%PDF-bbbb');
});

test('file names are reduced to a safe plain name', () => {
  assert.strictEqual(safePdfName('../../etc/passwd'), 'passwd.pdf');
  assert.strictEqual(safePdfName('C:\\dir\\report.PDF'), 'report.PDF');
  assert.strictEqual(safePdfName('a\u0007b\n<c>.pdf'), 'abc.pdf');
  assert.strictEqual(safePdfName(''), 'card.pdf');
  assert.ok(safePdfName('x'.repeat(500)).length <= 124);
});
