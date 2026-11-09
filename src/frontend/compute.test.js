const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const { spawnSync } = require("node:child_process");

const html = fs.readFileSync(`${__dirname}/public/index.html`, "utf8");
const start = html.indexOf("const capabilities =");
const end = html.indexOf("function toggleConnectivityScenario()", start);

function render(computePlatform, mode) {
  const element = () => ({
    dataset: {},
    classList: { add() {}, remove() {} },
    setAttribute() {},
    append(...children) { this.children = children; },
    appendChild(child) { this.children = [...(this.children || []), child]; },
    replaceChildren(...children) { this.children = children; },
    addEventListener() {},
  });
  const list = element();
  const context = {
    config: { computePlatform },
    document: { body: { dataset: {} }, createElement: element },
    redButtonEl: element(),
    redButtonLabelEl: element(),
    capabilityModeEl: element(),
    capabilityListEl: list,
  };
  vm.runInNewContext(`${html.slice(start, end)} renderCapabilities("${mode}");`, context);
  return list.children.find(card => card.children[1].children[0].textContent === "Compute");
}

for (const mode of ["connected", "disconnected"]) {
  test(`K3s stays yellow and AKS stays green when ${mode}`, () => {
    const local = render("k3s", mode);
    assert.equal(local.dataset.backing, "local");
    assert.equal(local.children[1].children[1].textContent, "K3s");
    const cloud = render("aks", mode);
    assert.equal(cloud.dataset.backing, "cloud");
    assert.equal(cloud.children[1].children[1].textContent, "AKS");
  });
}

test("compute platform configuration is exposed and invalid values fail startup", () => {
  const source = fs.readFileSync(`${__dirname}/server.js`, "utf8");
  assert.match(source, /computePlatform: config\.computePlatform/);
  const result = spawnSync(process.execPath, ["-e", 'require("./server")'], {
    cwd: __dirname, encoding: "utf8", env: { ...process.env, COMPUTE_PLATFORM: "invalid" },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /COMPUTE_PLATFORM must be aks or k3s/);
});

test("local and cloud cards retain their yellow and green styles", () => {
  assert.match(html, /\.capability-card\[data-backing="local"\]\s*\{[^}]*border-color: #a16207/);
  assert.match(html, /\.capability-card\[data-backing="cloud"\]\s*\{[^}]*border-color: #15803d/);
});
