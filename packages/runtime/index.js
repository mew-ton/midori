const { platform, arch } = process;

const PLATFORMS = {
  "win32-x64": "@midori/runtime-win32-x64",
  "darwin-x64": "@midori/runtime-darwin-x64",
  "darwin-arm64": "@midori/runtime-darwin-arm64",
  "linux-x64": "@midori/runtime-linux-x64",
};

const key = `${platform}-${arch}`;
const pkg = PLATFORMS[key];
if (!pkg) throw new Error(`Unsupported platform: ${key}`);

module.exports = require(pkg);
