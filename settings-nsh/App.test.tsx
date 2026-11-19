// `nsh test` 在无头 runner 里跑这个文件：页面可以渲染，侧栏和第一页的控件都在。
import { expect, open, test } from "nsh/test";

test("renders the settings window", async () => {
  const app = await open("./App.tsx");
  expect(app.texts()).toContain("ArchType");
  expect(app.texts()).toContain("常规");
  expect(app.texts()).toContain("候选词");
  expect(app.texts()).toContain("翻页键");
  expect(app.texts()).toContain("外观");
  expect(app.texts()).toContain("输入");
  expect(app.texts()).toContain("翻译");
});

test("shows the stats page", async () => {
  const app = await open("./App.tsx");
  await app.press("统计");
  expect(app.texts()).toContain("记录打字统计");
  expect(app.texts()).toContain("今日输入");
  expect(app.texts().join("")).toContain("按键热力图");
  await app.press("趋势");
  expect(app.texts()).toContain("每日趋势 · 近 30 天");
  expect(app.texts().join("")).toContain("最高一天");
  await app.press("方案");
  expect(app.texts()).toContain("方案 · 累计");
  await app.press("候选");
  expect(app.texts()).toContain("候选位置 · 累计");
});

test("fuzzy groups are hidden until the master switch is on", async () => {
  const app = await open("./App.tsx");
  await app.press("输入");
  expect(app.texts()).toContain("比如 n 和 l 不分");
  expect(app.texts()).not.toContain("z = zh");
});

test("offline model group appears for the local engine", async () => {
  const app = await open("./App.tsx");
  await app.press("翻译");
  expect(app.texts()).toContain("离线翻译模型");
  expect(app.texts().join("")).toContain("腾讯混元社区许可协议");
});
