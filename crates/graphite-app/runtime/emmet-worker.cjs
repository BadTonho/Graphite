'use strict';

const readline = require('node:readline');
const expand = require('./emmet.cjs').default;

const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
input.on('line', (line) => {
  try {
    const request = JSON.parse(line);
    const syntax = request.syntax || 'html';
    const options = syntax === 'css' || syntax === 'scss' || syntax === 'sass' || syntax === 'less'
      ? { syntax, type: 'stylesheet' }
      : { syntax };
    const value = expand(request.abbreviation, options);
    process.stdout.write(`${JSON.stringify({ value: typeof value === 'string' ? value : null })}\n`);
  } catch (error) {
    process.stdout.write(`${JSON.stringify({ error: String(error) })}\n`);
  }
});
