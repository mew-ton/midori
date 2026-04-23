const PLATFORM_PACKAGES = {
  "win32-x64": "@midori/runtime-win32-x64",
  "darwin-x64": "@midori/runtime-darwin-x64",
  "darwin-arm64": "@midori/runtime-darwin-arm64",
  "linux-x64": "@midori/runtime-linux-x64",
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORM_PACKAGES[key];

if (!pkg) {
  throw new Error(
    `@midori/runtime: unsupported platform "${key}". ` +
      `Supported: ${Object.keys(PLATFORM_PACKAGES).join(", ")}`,
  );
}

module.exports = require(pkg);
