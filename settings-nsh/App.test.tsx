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
  // 有记录时给「共 X 字符 · 最高一天 Y 字符」，没有时给提示：两种都算画对了。
  const trend = app.texts().join("");
  expect(trend.includes("最高一天") || trend.includes("这 30 天还没有输入记录")).toBe(true);
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

// 引擎种类(本地 / 哔哩哔哩 / 自定义)是用户配置,服务那一组的行随它显示或隐藏:
// 这里只验证与引擎无关的部分,免得测试在干净的 CI 上跟着默认配置一起失败。
test("translation page renders whatever the engine is", async () => {
  const app = await open("./App.tsx");
  await app.press("翻译");
  expect(app.texts()).toContain("互译");
  expect(app.texts()).toContain("正式语气");
  expect(app.texts().join("")).toContain("单按 Shift 切到「译」");
});

test("menu opens with the four actions", async () => {
  const app = await open("./App.tsx");
  await app.press("label:菜单");
  expect(app.texts()).toContain("恢复默认设置");
  expect(app.texts()).toContain("关于 ArchType");
  expect(app.texts()).toContain("导出日志…");
  expect(app.texts()).toContain("用文本编辑器打开自定义词");
});

test("window buttons take presses", async () => {
  const app = await open("./App.tsx");
  await app.press("label:最小化");
  await app.press("label:最大化");
});
