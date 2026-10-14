const assert = require('node:assert/strict');
const fs = require('node:fs');

function releaseVersion(base, runNumber) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(base);
  assert(match, 'Version must be stable major.minor.patch');
  assert(/^[1-9]\d*$/.test(runNumber), 'GITHUB_RUN_NUMBER must be a positive integer');
  const patch = Number(match[3]) + Number(runNumber);
  assert(Number.isSafeInteger(patch) && patch <= 65535, 'Release patch exceeds the Windows installer limit');
  assert(Number(match[1]) <= 255 && Number(match[2]) <= 255, 'Release major/minor exceeds the Windows installer limit');
  return `${match[1]}.${match[2]}.${patch}`;
}

if (process.argv.includes('--check')) {
  assert.equal(releaseVersion('0.1.0', '1'), '0.1.1');
  assert.equal(releaseVersion('1.2.3', '10'), '1.2.13');
  for (const [base, run] of [['0.1.0-beta', '1'], ['0.1.0', '0'], ['0.1.0', 'invalid'], ['0.1.65535', '1'], ['256.0.0', '1']]) {
    assert.throws(() => releaseVersion(base, run));
  }
  console.log('Automatic release version checks passed');
} else {
  const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
  const config = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
  const cargo = fs.readFileSync('src-tauri/Cargo.toml', 'utf8');
  const npmLock = JSON.parse(fs.readFileSync('package-lock.json', 'utf8'));
  const lockPath = 'src-tauri/Cargo.lock';
  const lock = fs.readFileSync(lockPath, 'utf8');
  assert.equal(config.version, pkg.version, 'Tauri and npm base versions must match');
  const current = cargo.match(/\[package\][\s\S]*?\nversion = "([^"]+)"/);
  assert.equal(current?.[1], pkg.version, 'Cargo and npm base versions must match');
  const lockedPackage = /name = "captrain"\r?\nversion = "([^"]+)"/;
  assert.equal(lock.match(lockedPackage)?.[1], pkg.version, 'Cargo.lock package is out of sync');
  assert.equal(npmLock.version, pkg.version, 'npm lock version is out of sync');
  assert.equal(npmLock.packages[''].version, pkg.version, 'npm lock root package is out of sync');
  const version = releaseVersion(pkg.version, process.env.GITHUB_RUN_NUMBER);
  pkg.version = config.version = version;
  fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
  fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(config, null, 2) + '\n');
  fs.writeFileSync('src-tauri/Cargo.toml', cargo.replace(/(\[package\][\s\S]*?\nversion = ")[^"]+"/, `$1${version}"`));
  // Only the local package version changes; all resolved dependencies stay locked.
  npmLock.version = npmLock.packages[''].version = version;
  fs.writeFileSync('package-lock.json', JSON.stringify(npmLock, null, 2) + '\n');
  fs.writeFileSync(lockPath, lock.replace(lockedPackage, match => match.replace(`version = "${current[1]}"`, `version = "${version}"`)));
  console.log(`Release version: ${version}`);
}
