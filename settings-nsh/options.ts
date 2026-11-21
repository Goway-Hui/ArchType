// 设置界面里各个下拉的选项、翻译服务商预设、模糊音分组等固定数据。
// 只放数据和小工具，不涉及界面渲染。
import { Config } from "archtype/native";

export const layoutItems = [
  { value: "Vertical", title: "竖排" },
  { value: "Horizontal", title: "横排" },
];

export const voiceKeyItems = [
  { value: "Alt_R", title: "右 Alt" },
  { value: "Control_R", title: "右 Ctrl" },
  { value: "Menu", title: "菜单键" },
  { value: "F8", title: "F8" },
  { value: "F9", title: "F9" },
  { value: "F10", title: "F10" },
];

// 录音设备（arecord -L 里的 ALSA PCM 名）：界面打开时列一次
export const micDeviceChoices: string[] = (() => {
  try {
    const value: unknown = JSON.parse(Config.voiceDevices() || "[]");
    return Array.isArray(value) ? value.filter((name): name is string => typeof name === "string") : [];
  } catch {
    return [];
  }
})();

export const candidateShadeItems = [
  { value: "system", title: "跟随系统" },
  { value: "light", title: "浅色" },
  { value: "dark", title: "深色" },
];

export const translateEngineItems = [
  { value: "local", title: "本地翻译（离线）" },
  { value: "bilibili", title: "哔哩哔哩（免费，不用 API Key）" },
  { value: "custom", title: "自定义翻译服务" },
];

export const translateEngineNotes: Record<string, string> = {
  local: "不联网、不用账号，打的字不出这台电脑；一句话等 1–3 秒",
  bilibili: "哔哩哔哩的 Index-Translate，不用注册；要联网，一句话不到 1 秒",
  custom: "填你自己的翻译接口（DeepSeek、通义千问、Ollama……），要联网",
};

export const biliModelItems = [
  { value: "Index-Translate-2B", title: "Index-Translate-2B（最快）" },
  { value: "Index-Translate-9B", title: "Index-Translate-9B" },
  { value: "Index-Translate-35B-A3B", title: "Index-Translate-35B-A3B（预览）" },
];

export const translateTargetItems = [
  { value: "en", title: "英语" },
  { value: "ja", title: "日语" },
  { value: "ko", title: "韩语" },
  { value: "fr", title: "法语" },
  { value: "de", title: "德语" },
  { value: "es", title: "西班牙语" },
  { value: "ru", title: "俄语" },
  { value: "pt", title: "葡萄牙语" },
  { value: "it", title: "意大利语" },
  { value: "vi", title: "越南语" },
  { value: "th", title: "泰语" },
  { value: "id", title: "印尼语" },
  { value: "ar", title: "阿拉伯语" },
];

export const translateProviders = [
  { title: "OpenAI", base: "https://api.openai.com/v1", model: "" },
  { title: "DeepSeek", base: "https://api.deepseek.com/v1", model: "deepseek-chat" },
  { title: "通义千问（阿里云百炼）", base: "https://dashscope.aliyuncs.com/compatible-mode/v1", model: "qwen-plus" },
  { title: "Kimi（月之暗面）", base: "https://api.moonshot.cn/v1", model: "moonshot-v1-8k" },
  { title: "智谱 GLM", base: "https://open.bigmodel.cn/api/paas/v4", model: "glm-4-flash" },
  { title: "硅基流动", base: "https://api.siliconflow.cn/v1", model: "" },
  { title: "OpenRouter", base: "https://openrouter.ai/api/v1", model: "" },
  { title: "Ollama（本机）", base: "http://127.0.0.1:11434/v1", model: "" },
  { title: "其他（自己填地址）", base: "", model: "" },
];

export const providerItems = translateProviders.map((provider, index) => ({
  value: String(index),
  title: provider.title,
}));

export function providerIndexOf(base: string): number {
  const trimmed = base.trim().replace(/\/+$/, "");
  const index = translateProviders.findIndex(
    (provider) => provider.base !== "" && trimmed.startsWith(provider.base),
  );
  return index >= 0 ? index : translateProviders.length - 1;
}

export const BILIBILI_URL = "https://index-translate.bilibili.com/v1";

export function engineOf(local: boolean, base: string): string {
  if (local) return "local";
  return base.includes("index-translate.bilibili.com") ? "bilibili" : "custom";
}

export const emojiKeyItems = [
  { value: "Super+period", title: "Super + ." },
  { value: "Control+semicolon", title: "Ctrl + ;" },
  { value: "Control+Shift+E", title: "Ctrl + Shift + E" },
  { value: "", title: "不用快捷键" },
];

export const fuzzyGroups: { key: string; label: string; default: boolean }[] = [
  { key: "FuzzyZh", label: "z = zh", default: true },
  { key: "FuzzyCh", label: "c = ch", default: true },
  { key: "FuzzySh", label: "s = sh", default: true },
  { key: "FuzzyNL", label: "n = l", default: false },
  { key: "FuzzyFH", label: "f = h", default: false },
  { key: "FuzzyRL", label: "r = l", default: false },
  { key: "FuzzyAn", label: "an = ang", default: true },
  { key: "FuzzyAnAi", label: "an = ai", default: false },
  { key: "FuzzyIan", label: "ian = iang", default: true },
  { key: "FuzzyUan", label: "uan = uang", default: true },
  { key: "FuzzyHuangWang", label: "huang = wang", default: false },
  { key: "FuzzyEn", label: "en = eng", default: true },
  { key: "FuzzyEngOng", label: "eng = ong", default: false },
  { key: "FuzzyIn", label: "in = ing", default: true },
  { key: "FuzzyUn", label: "un = ong", default: false },
  { key: "FuzzyUnIong", label: "un = iong", default: false },
];

export const arrowKeyItems = [
  { value: "Candidates", title: "都用来选词" },
  { value: "Caret", title: "左右键移动光标" },
];

export const tips: [string, string][] = [
  ["长句分段选", "先选开头的词，剩下的接着选"],
  ["忘掉学错的词", "光标移到候选上，按 Ctrl+Delete"],
  ["搜符号", "表情面板里打 gou → ✓、youjiantou → →"],
  ["互译", "单按 Shift 切到「译」，打中文出外文"],
];

export const issuesUrl = "https://github.com/Goway-Hui/ArchType/issues/new";

export const shuangpinItems = [
  { value: "Ziranma", title: "自然码" },
  { value: "Xiaohe", title: "小鹤双拼" },
  { value: "Microsoft", title: "微软双拼" },
  { value: "Sogou", title: "搜狗双拼" },
  { value: "ABC", title: "智能 ABC" },
  { value: "Ziguang", title: "紫光双拼" },
  { value: "PinyinJiajia", title: "拼音加加" },
];

export const shiftItems = [
  { value: "Both", title: "左右 Shift 都行" },
  { value: "Left", title: "左 Shift" },
  { value: "Right", title: "右 Shift" },
  { value: "Off", title: "不用 Shift 切换" },
];

// 另设的切换 / 互译快捷键:只列 fcitx5 没占的键(Ctrl+空格 是它切输入法用的)。
export const toggleKeyItems = [
  { value: "", title: "不用（单按 Shift 照常）" },
  { value: "Control+Shift+space", title: "Ctrl + Shift + 空格" },
];

export const translateModeKeyItems = [
  { value: "", title: "不用" },
  { value: "Control+Shift+Y", title: "Ctrl + Shift + Y" },
];

// 配置文件里写了别的键也认:补一项显示出来,不把它变成空。
export function withCurrentKey(
  items: { value: string; title: string }[],
  current: string,
): { value: string; title: string }[] {
  return current !== "" && !items.some((item) => item.value === current)
    ? [...items, { value: current, title: current }]
    : items;
}

export const initialModeItems = [
  { value: "Chinese", title: "中文" },
  { value: "English", title: "英文" },
  { value: "Last", title: "和上次切走时一样" },
];

export const windowModeItems = [
  { value: "Same", title: "和上次一样" },
  { value: "Chinese", title: "中文" },
  { value: "English", title: "英文" },
];

export const selectKeyItems = [
  { value: "Off", title: "不用" },
  { value: "Semicolon", title: "分号选第 2 个" },
  { value: "SemicolonQuote", title: "分号选第 2 个、引号选第 3 个" },
];

export const punctStyleItems = [
  { value: "Fullwidth", title: "全角 ，。" },
  { value: "Halfwidth", title: "半角 , ." },
];

export const themeItems = [
  { value: "system", title: "跟随系统" },
  { value: "light", title: "浅色" },
  { value: "dark", title: "深色" },
];
