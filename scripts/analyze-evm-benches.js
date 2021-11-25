#!/bin/bash node
const assert = require('assert');
const fs = require('fs');

const stdinBuffer = fs.readFileSync(0, 'utf-8'); // STDIN_FILENO = 0
assert(stdinBuffer);

const bench_path = stdinBuffer.toString().split('\n').filter(x=> !!x).slice(-1)[0];
assert(bench_path);


const bench_data = fs.readFileSync(bench_path, 'utf-8');
const bench_config = fs.readFileSync(__dirname + '/../resources/evm-benches.json', 'utf-8');

const benches = JSON.parse(bench_data);
const config = JSON.parse(bench_config);

let output = [];
benches.forEach(x => {
    const used_gas = config['benches'][x.name]['used_gas'];
    const total_weight = x.weight + x.reads * config.db.read + x.writes * config.db.write;
    const ratio = Number.parseInt((total_weight / used_gas).toString());
    output.push({ name: x.name, reads: x.reads, writes: x.writes, weight: x.weight, total_weight, used_gas, ratio });
});

// output = output.sort((a, b) => a.ratio - b.ratio);

function linearRegression(y, x){
    const lr = {};
    const n = y.length;
    let sum_x = 0;
    let sum_y = 0;
    let sum_xy = 0;
    let sum_xx = 0;
    let sum_yy = 0;

    for (let i = 0; i < y.length; i++) {
        sum_x += x[i];
        sum_y += y[i];
        sum_xy += x[i] * y[i];
        sum_xx += x[i] * x[i];
        sum_yy += y[i] * y[i];
    }

    lr['slope'] = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
    lr['intercept'] = (sum_y - lr.slope * sum_x) / n;
    lr['r2'] = Math.pow((n * sum_xy - sum_x * sum_y) / Math.sqrt((n * sum_xx - sum_x * sum_x) * (n * sum_yy - sum_y * sum_y)), 2);

    return lr;
}

const x = Array.from(Array(output.length).keys());
const y = output.map(x => x.ratio);

console.log(linearRegression(y, x));

console.table(output);