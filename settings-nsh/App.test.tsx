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
