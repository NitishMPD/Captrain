const assert = require('node:assert/strict');
const fs = require('node:fs');
const { test } = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(`${__dirname}/release-version.cjs`, 'utf8');
function fixture(newline = '\n') {
  return new Map(Object.entries({
    'package.json': '{"version":"0.1.0"}',
    'package-lock.json': '{"version":"0.1.0","packages":{"":{"version":"0.1.0"},"dependency":{"version":"2.0.0"}}}',
    'src-tauri/tauri.conf.json': '{"version":"0.1.0","identifier":"com.nitish.captrain"}',
    'src-tauri/Cargo.toml': ['[package]', 'name = "captrain"', 'version = "0.1.0"', '[dependencies]', 'other = "2"'].join(newline),
    'src-tauri/Cargo.lock': ['[[package]]', 'name = "captrain"', 'version = "0.1.0"', '[[package]]', 'name = "other"', 'version = "2.0.0"'].join(newline),
  }));
}
function run(files) {
  vm.runInNewContext(source, {
    require: name => name === 'node:fs' ? {
      readFileSync: path => { assert(files.has(path)); return files.get(path); },
      writeFileSync: (path, value) => files.set(path, value),
    } : require(name),
    process: { argv: [], env: { GITHUB_RUN_NUMBER: '10' } },
    console: { log() {} },
  });
}

for (const newline of ['\n', '\r\n']) test(`synchronizes all five version files with ${JSON.stringify(newline)}`, () => {
  const files = fixture(newline);
  run(files);
  for (const path of ['package.json', 'package-lock.json', 'src-tauri/tauri.conf.json']) assert.equal(JSON.parse(files.get(path)).version, '0.1.10');
  const npmLock = JSON.parse(files.get('package-lock.json'));
  assert.equal(npmLock.packages[''].version, '0.1.10');
  assert.equal(npmLock.packages.dependency.version, '2.0.0');
  assert(files.get('src-tauri/Cargo.toml').includes('version = "0.1.10"'));
  assert(files.get('src-tauri/Cargo.lock').includes('version = "0.1.10"'));
  assert(files.get('src-tauri/Cargo.lock').includes('version = "2.0.0"'));
});

test('rejects inconsistent versions before changing any file', () => {
  const files = fixture();
  files.set('src-tauri/Cargo.lock', files.get('src-tauri/Cargo.lock').replace('0.1.0', '0.2.0'));
  const before = new Map(files);
  assert.throws(() => run(files), /out of sync/);
  assert.deepEqual(files, before);
});
