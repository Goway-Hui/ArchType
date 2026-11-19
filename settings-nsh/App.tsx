import { mount, setColorScheme, colorScheme, computed, signal, Scroll, Text, View } from "nsh";
import { Button, Input, Select, Switch } from "nsh/base";
import type { JSX } from "nsh/jsx-runtime";
import { Config } from "archtype/native";

const pages = ["常规", "输入", "表情", "语音", "翻译", "自定义词", "统计", "更新"];

const lightUi = {
  page: "#f5f7f6",
  sidebar: "#e2ecfc",
  card: "#ffffff",
  border: "#e6eae8",
  text: "#1f2724",
  muted: "#59635e",
  accent: "#2F6FED",
  trackOff: "#cfd5d1",
};

const darkUi = {
  page: "#17191b",
  sidebar: "#1e2530",
  card: "#22262a",
  border: "#363b40",
  text: "#e6ece9",
  muted: "#9aa49f",
  accent: "#3B82F6",
  trackOff: "#3d4348",
};

const ui = computed(() => (colorScheme() === "dark" ? darkUi : lightUi));

const keySpecialNames: Record<string, string> = {
  Escape: "Esc",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backspace: "Backspace",
  Tab: "Tab",
  CapsLock: "Caps Lock",
  Enter: "Enter",
  ShiftLeft: "Shift",
  ShiftRight: "Shift",
  ControlLeft: "Ctrl",
  ControlRight: "Ctrl",
  AltLeft: "Alt",
  AltRight: "Alt",
  MetaLeft: "Super",
  MetaRight: "Super",
  ContextMenu: "Menu",
  Space: "Space",
  ArrowLeft: "左箭头",
  ArrowRight: "右箭头",
  ArrowUp: "上箭头",
  ArrowDown: "下箭头",
  Home: "Home",
  End: "End",
  PageUp: "Page Up",
  PageDown: "Page Down",
  Insert: "Insert",
  Delete: "Delete",
  NumLock: "Num Lock",
  NumpadEnter: "小键盘回车",
  NumpadAdd: "小键盘 +",
  NumpadSubtract: "小键盘 -",
  NumpadMultiply: "小键盘 *",
  NumpadDivide: "小键盘 /",
  NumpadDecimal: "小键盘 .",
  Fn: "Fn",
  IntlBackslash: "ISO \\",
  IntlRo: "ろ",
  IntlYen: "¥",
};

function keyLabel(code: string): string {
  if (code in keySpecialNames) return keySpecialNames[code];
  if (code.length === 4 && code.startsWith("Key")) return code[3];
  if (code.startsWith("Digit") && code.length === 6) return code[5];
  if (code.startsWith("Numpad") && code.length === 7 && code[6] >= "0" && code[6] <= "9") {
    return `小键盘 ${code[6]}`;
  }
  return code;
}

const kindNames: Record<string, string> = {
  han: "汉字",
  latin: "字母",
  number: "数字",
  punct: "标点",
  emoji: "emoji",
  other: "其他",
};

const sourceNames: Record<string, string> = {
  quanpin: "全拼",
  shuangpin: "双拼",
  english: "英文",
  translate: "互译",
};

const schemeNames: Record<string, string> = {
  quanpin: "全拼",
  shuangpin: "双拼",
  ziranma: "自然码",
  xiaohe: "小鹤双拼",
  microsoft: "微软双拼",
  sogou: "搜狗双拼",
  abc: "智能 ABC",
  ziguang: "紫光双拼",
  pinyinjiajia: "拼音加加",
};

const keyboardRows: { code: string | null; label: string; weight: number }[][] = [
  [
    { code: "Escape", label: "Esc", weight: 1 },
    { code: null, label: "", weight: 1 },
    { code: "F1", label: "F1", weight: 1 },
    { code: "F2", label: "F2", weight: 1 },
    { code: "F3", label: "F3", weight: 1 },
    { code: "F4", label: "F4", weight: 1 },
    { code: null, label: "", weight: 0.5 },
    { code: "F5", label: "F5", weight: 1 },
    { code: "F6", label: "F6", weight: 1 },
    { code: "F7", label: "F7", weight: 1 },
    { code: "F8", label: "F8", weight: 1 },
    { code: null, label: "", weight: 0.5 },
    { code: "F9", label: "F9", weight: 1 },
    { code: "F10", label: "F10", weight: 1 },
    { code: "F11", label: "F11", weight: 1 },
    { code: "F12", label: "F12", weight: 1 },
  ],
  [
    { code: "Backquote", label: "`", weight: 1 },
    { code: "Digit1", label: "1", weight: 1 },
    { code: "Digit2", label: "2", weight: 1 },
    { code: "Digit3", label: "3", weight: 1 },
    { code: "Digit4", label: "4", weight: 1 },
    { code: "Digit5", label: "5", weight: 1 },
    { code: "Digit6", label: "6", weight: 1 },
    { code: "Digit7", label: "7", weight: 1 },
    { code: "Digit8", label: "8", weight: 1 },
    { code: "Digit9", label: "9", weight: 1 },
    { code: "Digit0", label: "0", weight: 1 },
    { code: "Minus", label: "-", weight: 1 },
    { code: "Equal", label: "=", weight: 1 },
    { code: "Backspace", label: "Backspace", weight: 2 },
  ],
  [
    { code: "Tab", label: "Tab", weight: 1.5 },
    { code: "KeyQ", label: "Q", weight: 1 },
    { code: "KeyW", label: "W", weight: 1 },
    { code: "KeyE", label: "E", weight: 1 },
    { code: "KeyR", label: "R", weight: 1 },
    { code: "KeyT", label: "T", weight: 1 },
    { code: "KeyY", label: "Y", weight: 1 },
    { code: "KeyU", label: "U", weight: 1 },
    { code: "KeyI", label: "I", weight: 1 },
    { code: "KeyO", label: "O", weight: 1 },
    { code: "KeyP", label: "P", weight: 1 },
    { code: "BracketLeft", label: "[", weight: 1 },
    { code: "BracketRight", label: "]", weight: 1 },
    { code: "Backslash", label: "\\", weight: 1.5 },
  ],
  [
    { code: "CapsLock", label: "Caps Lock", weight: 1.75 },
    { code: "KeyA", label: "A", weight: 1 },
    { code: "KeyS", label: "S", weight: 1 },
    { code: "KeyD", label: "D", weight: 1 },
    { code: "KeyF", label: "F", weight: 1 },
    { code: "KeyG", label: "G", weight: 1 },
    { code: "KeyH", label: "H", weight: 1 },
    { code: "KeyJ", label: "J", weight: 1 },
    { code: "KeyK", label: "K", weight: 1 },
    { code: "KeyL", label: "L", weight: 1 },
    { code: "Semicolon", label: ";", weight: 1 },
    { code: "Quote", label: "'", weight: 1 },
    { code: "Enter", label: "Enter", weight: 2.25 },
  ],
  [
    { code: "ShiftLeft", label: "Shift", weight: 2.25 },
    { code: "KeyZ", label: "Z", weight: 1 },
    { code: "KeyX", label: "X", weight: 1 },
    { code: "KeyC", label: "C", weight: 1 },
    { code: "KeyV", label: "V", weight: 1 },
    { code: "KeyB", label: "B", weight: 1 },
    { code: "KeyN", label: "N", weight: 1 },
    { code: "KeyM", label: "M", weight: 1 },
    { code: "Comma", label: ",", weight: 1 },
    { code: "Period", label: ".", weight: 1 },
    { code: "Slash", label: "/", weight: 1 },
    { code: "ShiftRight", label: "Shift", weight: 2.75 },
  ],
  [
    { code: "ControlLeft", label: "Ctrl", weight: 1.25 },
    { code: "MetaLeft", label: "Super", weight: 1.25 },
    { code: "AltLeft", label: "Alt", weight: 1.25 },
    { code: "Space", label: "Space", weight: 6.25 },
    { code: "AltRight", label: "Alt", weight: 1.25 },
    { code: "MetaRight", label: "Super", weight: 1.25 },
    { code: "ContextMenu", label: "Menu", weight: 1.25 },
    { code: "ControlRight", label: "Ctrl", weight: 1.25 },
  ],
];

const keyboardCodes = new Set(
  keyboardRows.flatMap((row) => row.map((key) => key.code).filter((code): code is string => code !== null)),
);

function maxKeyCount(keys: Record<string, number>): number {
  return Math.max(1, ...Object.values(keys));
}

function otherKeys(keys: Record<string, number>): { label: string; count: number }[] {
  return Object.entries(keys)
    .filter(([code]) => !keyboardCodes.has(code))
    .sort((a, b) => b[1] - a[1])
    .map(([code, count]) => ({ label: keyLabel(code), count }));
}

function countRows(
  counts: Record<string, number>,
  names: Record<string, string>,
): { label: string; count: number }[] {
  return Object.entries(counts)
    .sort((a, b) => b[1] - a[1])
    .map(([key, count]) => ({ label: names[key] ?? key, count }));
}

function SmallButton(props: {
  title: string;
  onPress: () => void;
  tone?: "accent" | "plain";
  disabled?: boolean;
}) {
  return (
    <View
      style={{
        backgroundColor: props.tone === "accent" ? ui().accent : ui().card,
        borderWidth: 1,
        borderColor: props.tone === "accent" ? ui().accent : ui().border,
        borderRadius: 8,
        opacity: props.disabled ? 0.5 : 1,
      }}
    >
      <Button onPress={props.onPress} disabled={props.disabled}>
        <Text
          style={{
            color: props.tone === "accent" ? "white" : ui().text,
            fontSize: 13,
            paddingLeft: 12,
            paddingRight: 12,
            paddingTop: 4,
            paddingBottom: 4,
          }}
        >
          {props.title}
        </Text>
      </Button>
    </View>
  );
}

function InputBox(props: {
  value: string;
  placeholder?: string;
  masked?: boolean;
  width?: number;
  onChange: (value: string) => void;
}) {
  return (
    <View
      style={{
        backgroundColor: ui().page,
        borderWidth: 1,
        borderColor: ui().border,
        borderRadius: 8,
        width: props.width ?? 280,
      }}
    >
      <Input
        value={props.value}
        placeholder={props.placeholder}
        masked={props.masked}
        style={{
          color: ui().text,
          paddingLeft: 10,
          paddingRight: 10,
          paddingTop: 6,
          paddingBottom: 6,
          fontSize: 13,
        }}
        onChange={(event) => props.onChange(event.value)}
      />
    </View>
  );
}

function Metric(props: { title: string; value: string; unit?: string; note?: string }) {
  return (
    <View style={{ flex: 1, gap: 2, paddingTop: 6, paddingBottom: 6 }}>
      <Text style={{ color: ui().muted, fontSize: 12 }}>{props.title}</Text>
      <View style={{ flexDirection: "row", alignItems: "flex-end", gap: 4 }}>
        <Text style={{ color: ui().text, fontSize: 24, fontWeight: "800" }}>{props.value}</Text>
        {props.unit ? <Text style={{ color: ui().muted, fontSize: 12 }}>{props.unit}</Text> : null}
      </View>
      {props.note ? <Text style={{ color: ui().muted, fontSize: 11 }}>{props.note}</Text> : null}
    </View>
  );
}

function heatColor(count: number, max: number): string {
  if (count <= 0 || max <= 0) return colorScheme() === "dark" ? "#343a37" : "#eef1f0";
  const level = Math.max(1, Math.min(4, Math.ceil((count * 4) / max)));
  if (colorScheme() === "dark") {
    return ["#343a37", "rgba(110, 168, 254, 0.22)", "rgba(110, 168, 254, 0.42)",
      "rgba(110, 168, 254, 0.68)", "#6EA8FE"][level];
  }
  return ["#eef1f0", "rgba(47, 111, 237, 0.20)", "rgba(47, 111, 237, 0.40)",
    "rgba(47, 111, 237, 0.65)", "#2F6FED"][level];
}

function KeyCap(props: { label: string; weight: number; count: number; max: number }) {
  const level = props.count <= 0 ? 0 : Math.max(1, Math.min(4, Math.ceil((props.count * 4) / props.max)));
  const background = heatColor(props.count, props.max);
  return (
    <View
      style={{
        width: props.weight * 36 - 4,
        height: 32,
        borderRadius: 5,
        backgroundColor: background,
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <Text style={{ color: level >= 3 ? (colorScheme() === "dark" ? "#0B1B33" : "white") : ui().text, fontSize: 11 }}>
        {props.label}
      </Text>
    </View>
  );
}

function KeyChip(props: { label: string; count: number; max: number }) {
  return (
    <View
      style={{
        flexDirection: "row",
        alignItems: "center",
        gap: 6,
        borderWidth: 1,
        borderColor: ui().border,
        borderRadius: 6,
        paddingLeft: 7,
        paddingRight: 7,
        paddingTop: 3,
        paddingBottom: 3,
      }}
    >
      <View
        style={{
          width: 10,
          height: 10,
          borderRadius: 2,
          backgroundColor: heatColor(props.count, props.max),
        }}
      />
      <Text style={{ color: ui().text, fontSize: 12 }}>{props.label}</Text>
      <Text style={{ color: ui().muted, fontSize: 11 }}>{formatNumber(props.count)}</Text>
    </View>
  );
}

function Card(props: { children: JSX.Element }) {
  return (
    <View
      style={{
        backgroundColor: ui().card,
        borderRadius: 12,
        borderWidth: 1,
        borderColor: ui().border,
        paddingLeft: 20,
        paddingRight: 20,
        paddingTop: 18,
        paddingBottom: 18,
      }}
    >
      {props.children}
    </View>
  );
}

function Row(props: { title: string; subtitle?: string; children?: JSX.Element }) {
  return (
    <View
      style={{
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "space-between",
        paddingTop: 10,
        paddingBottom: 10,
        gap: 16,
      }}
    >
      <View style={{ flex: 1, gap: 2 }}>
        <Text style={{ color: ui().text, fontSize: 15 }}>{props.title}</Text>
        {props.subtitle ? (
          <Text style={{ color: ui().muted, fontSize: 12 }}>{props.subtitle}</Text>
        ) : null}
      </View>
      {props.children}
    </View>
  );
}

function SelectBox(props: {
  items: { value: string; title: string }[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <Select
      items={props.items}
      value={props.value}
      onChange={(e) => props.onChange(e.value ?? "")}
      cover={false}
      align="end"
      offset={6}
      popupStyle={{
        backgroundColor: ui().card,
        borderWidth: 1,
        borderColor: ui().border,
        borderRadius: 10,
        padding: 4,
        minWidth: 160,
      }}
      renderItem={(entry) => {
        const chosen = entry.value === props.value;
        return (
          <View
            style={{
              flexDirection: "row",
              alignItems: "center",
              justifyContent: "space-between",
              gap: 12,
              paddingLeft: 10,
              paddingRight: 10,
              paddingTop: 8,
              paddingBottom: 8,
              borderRadius: 6,
              backgroundColor: chosen ? ui().sidebar : ui().card,
            }}
          >
            <Text
              style={{
                color: chosen ? ui().accent : ui().text,
                fontSize: 13,
                fontWeight: chosen ? "700" : "400",
              }}
            >
              {entry.title}
            </Text>
            {chosen ? <Text style={{ color: ui().accent, fontSize: 12 }}>✓</Text> : null}
          </View>
        );
      }}
      trigger={
        <View
          style={{
            flexDirection: "row",
            alignItems: "center",
            gap: 8,
            paddingTop: 6,
            paddingBottom: 6,
            paddingLeft: 10,
            paddingRight: 10,
            borderRadius: 6,
            borderWidth: 1,
            borderColor: ui().border,
            backgroundColor: ui().card,
            minWidth: 120,
            justifyContent: "space-between",
          }}
        >
          <Text style={{ color: ui().text, fontSize: 13 }}>
            {props.items.find((item) => item.value === props.value)?.title ?? "选择"}
          </Text>
          <Text style={{ color: ui().muted, fontSize: 10 }}>▾</Text>
        </View>
      }
    />
  );
}

/* 带加减号的数字输入：nsh 的 NumberInput 把数字夹在两个按钮中间，自己排一行。 */
function StepInput(props: {
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  // 样式写成函数:ui() 在 View 的属性求值时读,主题一换就重算。
  const buttonStyle = () => ({
    width: 24,
    height: 24,
    alignItems: "center" as const,
    justifyContent: "center" as const,
    backgroundColor: ui().card,
    borderWidth: 1,
    borderColor: ui().border,
    borderRadius: 6,
  });
  return (
    <View style={{ flexDirection: "row", alignItems: "center" }}>
      <View style={buttonStyle()}>
        <Button onPress={() => props.onChange(Math.max(props.min, props.value - 1))}>
          <Text style={{ color: ui().text, fontSize: 15 }}>−</Text>
        </Button>
      </View>
      <Text style={{ color: ui().text, fontSize: 13, width: 34, textAlign: "center" }}>
        {props.value}
      </Text>
      <View style={buttonStyle()}>
        <Button onPress={() => props.onChange(Math.min(props.max, props.value + 1))}>
          <Text style={{ color: ui().text, fontSize: 15 }}>+</Text>
        </Button>
      </View>
    </View>
  );
}

function RadioDot(props: { selected: boolean }) {
  return (
    <View
      style={{
        width: 18,
        height: 18,
        borderRadius: 9,
        borderWidth: 2,
        borderColor: props.selected ? ui().accent : ui().muted,
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {props.selected ? (
        <View style={{ width: 8, height: 8, borderRadius: 4, backgroundColor: ui().accent }} />
      ) : null}
    </View>
  );
}

function Toggle(props: { checked: boolean; disabled?: boolean; onChange: (on: boolean) => void }) {
  return (
    <Switch
      checked={props.checked}
      disabled={props.disabled ?? false}
      onChange={(e) => props.onChange(e.checked)}
      trackStyle={{
        width: 40,
        height: 22,
        borderRadius: 11,
        padding: 2,
        backgroundColor: props.checked ? ui().accent : ui().trackOff,
      }}
      thumbStyle={{
        width: 18,
        height: 18,
        borderRadius: 9,
        backgroundColor: "white",
        marginLeft: props.checked ? 18 : 0,
      }}
    />
  );
}

const layoutItems = [
  { value: "Vertical", title: "竖排" },
  { value: "Horizontal", title: "横排" },
];

const voiceKeyItems = [
  { value: "Alt_R", title: "右 Alt" },
  { value: "Control_R", title: "右 Ctrl" },
  { value: "Menu", title: "菜单键" },
  { value: "F8", title: "F8" },
  { value: "F9", title: "F9" },
  { value: "F10", title: "F10" },
];

const translateEngineItems = [
  { value: "local", title: "本地翻译（离线）" },
  { value: "bilibili", title: "哔哩哔哩（免费，不用 API Key）" },
  { value: "custom", title: "自定义翻译服务" },
];

const translateEngineNotes: Record<string, string> = {
  local: "不联网、不用账号，打的字不出这台电脑；一句话等 1–3 秒",
  bilibili: "哔哩哔哩的 Index-Translate，不用注册；要联网，一句话不到 1 秒",
  custom: "填你自己的翻译接口（DeepSeek、通义千问、Ollama……），要联网",
};

const biliModelItems = [
  { value: "Index-Translate-2B", title: "Index-Translate-2B（最快）" },
  { value: "Index-Translate-9B", title: "Index-Translate-9B" },
  { value: "Index-Translate-35B-A3B", title: "Index-Translate-35B-A3B（预览）" },
];

const translateTargetItems = [
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

const translateProviders = [
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

const providerItems = translateProviders.map((provider, index) => ({
  value: String(index),
  title: provider.title,
}));

function providerIndexOf(base: string): number {
  const trimmed = base.trim().replace(/\/+$/, "");
  const index = translateProviders.findIndex(
    (provider) => provider.base !== "" && trimmed.startsWith(provider.base),
  );
  return index >= 0 ? index : translateProviders.length - 1;
}

const BILIBILI_URL = "https://index-translate.bilibili.com/v1";

function engineOf(local: boolean, base: string): string {
  if (local) return "local";
  return base.includes("index-translate.bilibili.com") ? "bilibili" : "custom";
}

const emojiKeyItems = [
  { value: "Super+period", title: "Super + ." },
  { value: "Control+semicolon", title: "Ctrl + ;" },
  { value: "Control+Shift+E", title: "Ctrl + Shift + E" },
  { value: "", title: "不用快捷键" },
];

const fuzzyGroups: { key: string; label: string; default: boolean }[] = [
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

const arrowKeyItems = [
  { value: "Candidates", title: "都用来选词" },
  { value: "Caret", title: "左右键移动光标" },
];

const tips: [string, string][] = [
  ["长句分段选", "先选开头的词，剩下的接着选"],
  ["忘掉学错的词", "光标移到候选上，按 Ctrl+Delete"],
  ["搜符号", "表情面板里打 gou → ✓、youjiantou → →"],
  ["互译", "单按 Shift 切到「译」，打中文出外文"],
];

const issuesUrl = "https://github.com/Goway-Hui/ArchType/issues/new";

const inputSchemeItems = [
  { value: "Quanpin", title: "拼音输入" },
  { value: "Shuangpin", title: "双拼输入" },
  { value: "Wubi86", title: "五笔输入" },
];

const shuangpinItems = [
  { value: "Ziranma", title: "自然码" },
  { value: "Xiaohe", title: "小鹤双拼" },
  { value: "Microsoft", title: "微软双拼" },
  { value: "Sogou", title: "搜狗双拼" },
  { value: "ABC", title: "智能 ABC" },
  { value: "Ziguang", title: "紫光双拼" },
  { value: "PinyinJiajia", title: "拼音加加" },
];

const shiftItems = [
  { value: "Both", title: "左右 Shift 都行" },
  { value: "Left", title: "左 Shift" },
  { value: "Right", title: "右 Shift" },
  { value: "Off", title: "不用 Shift 切换" },
];

const initialModeItems = [
  { value: "Chinese", title: "中文" },
  { value: "English", title: "英文" },
  { value: "Last", title: "和上次切走时一样" },
];

const selectKeyItems = [
  { value: "Off", title: "不用" },
  { value: "Semicolon", title: "分号选第 2 个" },
  { value: "SemicolonQuote", title: "分号选第 2 个、引号选第 3 个" },
];

const punctStyleItems = [
  { value: "Fullwidth", title: "全角 ，。" },
  { value: "Halfwidth", title: "半角 , ." },
];

const themeItems = [
  { value: "system", title: "跟随系统" },
  { value: "light", title: "浅色" },
  { value: "dark", title: "深色" },
];

interface StatsDay {
  chars?: number;
  activeMs?: number;
  keys?: Record<string, number>;
  sources?: Record<string, number>;
  schemes?: Record<string, number>;
  kinds?: Record<string, number>;
  selections?: number[];
  beyond?: number;
}

interface StatsSummary {
  todayChars: number;
  totalChars: number;
  todayActive: number;
  totalActive: number;
  todayReadable: number;
  totalReadable: number;
  todaySpeed: number;
  avgSpeed: number;
  bestChars: number;
  bestDay: string;
  fastestSpeed: number;
  fastestDay: string;
  streak: number;
  longest: number;
  recordedDays: number;
  totalKeys: number;
  keys: Record<string, number>;
  kinds: Record<string, number>;
  todayKinds: Record<string, number>;
  sources: Record<string, number>;
  todaySources: Record<string, number>;
  schemes: Record<string, number>;
  todaySchemes: Record<string, number>;
  selections: number[];
  todaySelections: number[];
  beyond: number;
  todayBeyond: number;
  recent: { day: string; chars: number }[];
}

interface StatsFile {
  days?: Record<string, StatsDay>;
}

function isStatsFile(value: unknown): value is StatsFile {
  return typeof value === "object" && value !== null && "days" in value;
}

function parseStats(text: string): StatsFile {
  if (!text) return {};
  try {
    const value: unknown = JSON.parse(text);
    return isStatsFile(value) ? value : {};
  } catch {
    return {};
  }
}

function addCounts(target: Record<string, number>, source?: Record<string, number>): void {
  for (const [name, count] of Object.entries(source ?? {})) {
    target[name] = (target[name] ?? 0) + count;
  }
}

function readableOf(day: StatsDay | undefined): number {
  const kinds = day?.kinds ?? {};
  return (kinds.han ?? 0) + (kinds.latin ?? 0);
}

function shiftDay(key: string, offset: number): string {
  const [year, month, day] = key.split("-").map(Number);
  return dayKey(new Date(year, month - 1, day + offset));
}

function summarizeStats(days: Record<string, StatsDay>, today: string): StatsSummary {
  const recorded = Object.keys(days).sort();
  const day = days[today] ?? {};
  const keys: Record<string, number> = {};
  const kinds: Record<string, number> = {};
  const sources: Record<string, number> = {};
  const schemes: Record<string, number> = {};
  const selections = [0, 0, 0, 0, 0, 0, 0, 0, 0];
  let totalChars = 0;
  let totalActive = 0;
  let totalReadable = 0;
  let totalKeys = 0;
  let beyond = 0;
  let bestChars = 0;
  let bestDay = "";
  let fastestSpeed = 0;
  let fastestDay = "";
  for (const key of recorded) {
    const value = days[key] ?? {};
    const chars = value.chars ?? 0;
    const active = value.activeMs ?? 0;
    const readable = readableOf(value);
    totalChars += chars;
    totalKeys += Object.values(value.keys ?? {}).reduce((sum, count) => sum + count, 0);
    if (active > 0) {
      totalActive += active;
      totalReadable += readable;
    }
    if (chars > bestChars) {
      bestChars = chars;
      bestDay = key;
    }
    if (active >= 60000) {
      const cpm = readable / (active / 60000);
      if (cpm > fastestSpeed) {
        fastestSpeed = cpm;
        fastestDay = key;
      }
    }
    addCounts(keys, value.keys);
    addCounts(kinds, value.kinds);
    addCounts(sources, value.sources);
    addCounts(schemes, value.schemes);
    (value.selections ?? []).forEach((count, index) => {
      if (index < 9) selections[index] += count;
    });
    beyond += value.beyond ?? 0;
  }
  const present = new Set(recorded);
  let cursor = present.has(today) ? today : shiftDay(today, -1);
  let streak = 0;
  while (present.has(cursor)) {
    streak += 1;
    cursor = shiftDay(cursor, -1);
  }
  let longest = 0;
  let run = 0;
  let previous = "";
  for (const key of recorded) {
    run = previous !== "" && shiftDay(previous, 1) === key ? run + 1 : 1;
    longest = Math.max(longest, run);
    previous = key;
  }
  const recent: { day: string; chars: number }[] = [];
  for (let offset = 29; offset >= 0; offset -= 1) {
    const key = shiftDay(today, -offset);
    recent.push({ day: key, chars: days[key]?.chars ?? 0 });
  }
  const todayActive = day.activeMs ?? 0;
  const todayReadable = readableOf(day);
  const todayChars = day.chars ?? 0;
  return {
    todayChars,
    totalChars,
    todayActive,
    totalActive,
    todayReadable,
    totalReadable,
    todaySpeed: todayActive > 0 ? Math.round(todayReadable / (todayActive / 60000)) : 0,
    avgSpeed: totalActive > 0 ? Math.round(totalReadable / (totalActive / 60000)) : 0,
    bestChars,
    bestDay,
    fastestSpeed: Math.round(fastestSpeed),
    fastestDay,
    streak,
    longest,
    recordedDays: recorded.length,
    totalKeys,
    keys,
    kinds,
    todayKinds: day.kinds ?? {},
    sources,
    todaySources: day.sources ?? {},
    schemes,
    todaySchemes: day.schemes ?? {},
    selections,
    todaySelections: day.selections ?? [],
    beyond,
    todayBeyond: day.beyond ?? 0,
    recent,
  };
}

function versionNewer(candidate: string, installed: string): boolean {
  const parse = (text: string) =>
    text
      .trim()
      .replace(/^[vV]/, "")
      .split("-")[0]
      .match(/\d+/g)
      ?.map(Number) ?? [];
  const left = parse(candidate);
  const right = parse(installed);
  for (let i = 0; i < Math.max(left.length, right.length); i += 1) {
    const a = left[i] ?? 0;
    const b = right[i] ?? 0;
    if (a !== b) return a > b;
  }
  return false;
}

function humanSize(bytes: number): string {
  return bytes >= 1e9 ? `${(bytes / 1e9).toFixed(2)} GB` : `${Math.round(bytes / 1e6)} MB`;
}

function formatNumber(value: number): string {
  return Math.round(value)
    .toString()
    .replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

function formatActive(milliseconds: number): string {
  if (milliseconds <= 0) return "0 分";
  const minutes = Math.floor(milliseconds / 60000);
  if (minutes === 0) return `${Math.max(1, Math.round(milliseconds / 1000))} 秒`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) return `${rest} 分`;
  return rest === 0 ? `${hours} 小时` : `${hours} 小时 ${rest} 分`;
}

function dayKey(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function boolOf(value: string, fallback: boolean): boolean {
  if (value === "True") return true;
  if (value === "False") return false;
  return fallback;
}

let voicePollStarted = false;

function App() {
  const current = signal(0);

  const layout = signal(Config.get("CandidateLayout") || "Vertical");
  const pageSize = signal(Number(Config.get("PageSize") || "9"));
  const theme = signal(Config.stateGet("theme") || "system");
  const blue = signal(Config.classicuiGet("Theme") === "archtype");
  const fontSize = signal(Number(Config.fontSize() || "10"));
  const inputScheme = signal(Config.get("InputScheme") || "Quanpin");
  const shuangpin = signal(Config.get("ShuangpinScheme") || "Ziranma");
  const typo = signal(boolOf(Config.get("TypoCorrection"), true));
  const shiftToggle = signal(Config.get("ShiftToggle") || "Both");
  const shiftConflicts = signal(Config.shiftConflicts());
  const shiftFreed = signal(Config.shiftFreeState());
  const shiftFreeOnState = signal(shiftFreed() !== "");
  const shiftConflictVisible = computed(
    () => shiftToggle() !== "Off" && (shiftConflicts() !== "" || shiftFreed() !== ""),
  );
  const shiftConflictText = computed(() => {
    const names = shiftConflicts()
      .split("\n")
      .map((line) => line.split("=")[1] ?? "")
      .filter((key) => key !== "")
      .map((key) => (key === "Shift_L" ? "左 Shift" : key === "Shift_R" ? "右 Shift" : key));
    const unique = Array.from(new Set(names));
    return unique.length > 0 ? `${unique.join("、")} 已被 fcitx5 的「临时切换输入法」占用` : "";
  });
  const autoPair = signal(boolOf(Config.get("AutoPairPunct"), false));
  const fuzzyOn = signal(boolOf(Config.get("Fuzzy"), false));
  const fuzzy = signal<Record<string, boolean>>(
    Object.fromEntries(fuzzyGroups.map((group) => [group.key, boolOf(Config.get(group.key), group.default)])),
  );
  const emojiSuggest = signal(boolOf(Config.get("EmojiSuggest"), true));
  const emojiKey = signal(Config.get("EmojiKey") || "Super+period");
  const voiceOn = signal(boolOf(Config.get("Voice"), true));
  const voiceKey = signal(Config.get("VoiceKey") || "Alt_R");
  const voiceCopy = signal(boolOf(Config.get("VoiceCopy"), false));
  const voiceModel = signal(Config.voiceModel());
  const translateOn = signal(boolOf(Config.get("Translate"), false));
  const translateLocal = signal(boolOf(Config.get("TranslateLocal"), true));
  const translateBase = signal(Config.get("TranslateBaseUrl") || "");
  const translateTarget = signal(Config.get("TranslateTarget") || "en");
  const customWords = signal(Config.customWords());
  const translateModel = signal(Config.get("TranslateModel") || "");
  const translateFormal = signal(boolOf(Config.get("TranslateFormal"), false));
  const translateKey = signal(Config.translateKey());
  const translateProvider = signal(providerIndexOf(Config.get("TranslateBaseUrl") || ""));
  const testState = signal("");
  const customWord = signal("");
  const customPinyin = signal("");
  const customMessage = signal("");
  const customSearch = signal("");
  const customPinyinEdited = signal(false);
  const voiceState = signal(Config.voiceDownloadState());
  const voiceProgress = signal(Config.voiceDownloadProgress());
  const voicePartial = signal(Config.voicePartialBytes());
  const voiceBusy = computed(
    () =>
      voiceState().startsWith("downloading") ||
      voiceState().startsWith("verifying") ||
      voiceState().startsWith("extracting"),
  );
  const localMissing = signal(Config.localMissing());
  const localState = signal(Config.localDownloadState());
  const localProgress = signal(Config.localDownloadProgress());
  const localBusy = computed(() => localState().startsWith("downloading") || localState().startsWith("installing"));
  const localSubtitle = computed(() => {
    const state = localState();
    const [doneText, totalText] = localProgress().split("/");
    const done = Number(doneText) || 0;
    const total = Number(totalText) || 0;
    if (state === "downloading" && total > 0) {
      return `正在下载模型：${humanSize(done)} / ${humanSize(total)}（${Math.floor((done * 100) / total)}%）`;
    }
    if (state === "downloading") return "正在连接…";
    if (state === "installing") return "正在安装运行程序…";
    if (state === "done") return "已完成";
    if (state.startsWith("error:")) return `没下载好：${state.slice(6)}`;
    if (localMissing() === "ready") return "模型和运行程序都已就绪";
    const missing = localMissing();
    if (missing === "model") return "还差模型（约 1.1 GB）";
    if (missing === "llama") return "还差运行程序（约 23 MB）";
    return "要下载模型（约 1.1 GB）和运行程序（约 23 MB），翻译时占 1.5 GB 左右内存";
  });
  const voiceModelSubtitle = computed(() => {
    const state = voiceState();
    const [doneText, totalText] = voiceProgress().split("/");
    const done = Number(doneText) || 0;
    const total = Number(totalText) || 0;
    if (state === "downloading" && total > 0) {
      return `正在下载：${humanSize(done)} / ${humanSize(total)}（${Math.floor((done * 100) / total)}%）`;
    }
    if (state === "downloading") return "正在连接…";
    if (state === "verifying") return "正在校验…";
    if (state === "extracting") return "正在解包…";
    if (state === "done") return "已完成";
    if (state.startsWith("error:")) return `没下载好：${state.slice(6)}`;
    if (voiceModel()) return "已下载";
    const partial = Number(voicePartial()) || 0;
    if (partial > 0) return `下了 ${humanSize(partial)} / ${humanSize(102920639)}，可以接着下`;
    return "在本机识别，录音不上传。要下载 102.9 MB，解开后约 130 MB";
  });
  const checkUpdate = signal(boolOf(Config.get("CheckUpdate"), true));
  const betaUpdates = signal(boolOf(Config.get("BetaUpdates"), false));
  const updateNotes = signal("");
  const updateTag = signal("");
  const updateTitle = computed(() => (updateTag() ? `有新版本 ${updateTag()}` : "最新版本"));
  const updateSource = Config.installSource();
  const updateState = signal("");
  const updateUrl = signal("");

  if (!voicePollStarted) {
    voicePollStarted = true;
    setInterval(() => {
      const state = Config.voiceDownloadState();
      voiceState.set(state);
      voiceProgress.set(Config.voiceDownloadProgress());
      voicePartial.set(Config.voicePartialBytes());
      const local = Config.localDownloadState();
      localState.set(local);
      if (lexiconBusy()) {
        lexiconBusyState.set(Config.lexiconDownloadState());
        lexiconProgress.set(Config.lexiconDownloadProgress());
        if (Config.lexiconDownloadState().startsWith("done:")) refreshLexicon();
      }
      localProgress.set(Config.localDownloadProgress());
      if (local === "done") localMissing.set(Config.localMissing());
      if (state === "done") voiceModel.set(Config.voiceModel());
    }, 1000);
  }

  const statsRaw = signal(Config.stats());
  const statsTab = signal("keys");
  const typingStats = signal(boolOf(Config.get("TypingStats"), false));
  const clearState = signal("");
  const summary = computed(() =>
    summarizeStats(parseStats(statsRaw()).days ?? {}, dayKey(new Date())),
  );
  const trendPeak = computed(() =>
    Math.max(0, ...summary().recent.map((day) => day.chars)),
  );
  const trendTotal = computed(() =>
    summary().recent.reduce((sum, day) => sum + day.chars, 0),
  );

  const shuangpinShowKeys = signal(boolOf(Config.get("ShuangpinShowKeys"), false));
  const wubiAutoCommit = signal(boolOf(Config.get("WubiAutoCommit"), true));
  const wubiHint = signal(boolOf(Config.get("WubiHint"), true));
  const initialMode = signal(Config.get("InitialMode") || "Chinese");
  const traditional = signal(boolOf(Config.get("Traditional"), false));
  const rareChars = signal(boolOf(Config.get("RareExtChars"), false));
  const punctStyle = signal(Config.get("PunctuationStyle") || "Fullwidth");
  const dateTime = signal(boolOf(Config.get("DateTime"), true));
  const calculator = signal(boolOf(Config.get("Calculator"), true));
  const toolbar = signal(boolOf(Config.get("Toolbar"), true));
  const clipboardHistory = signal(boolOf(Config.get("ClipboardHistory"), false));
  const selectKeys = signal(Config.get("SelectKeys") || "Off");
  const pickChar = signal(boolOf(Config.get("PickChar"), true));
  const association = signal(boolOf(Config.get("Association"), false));
  const autoSpace = signal(boolOf(Config.get("AutoSpace"), false));
  const autoSpaceLetters = signal(boolOf(Config.get("AutoSpaceLetters"), true));
  const autoSpaceDigits = signal(boolOf(Config.get("AutoSpaceDigits"), true));
  const calcHint = computed(() =>
    inputScheme() === "Shuangpin" || inputScheme() === "Wubi86"
      ? `${inputScheme() === "Shuangpin" ? "双拼" : "五笔"}时打大写 V：V1+2*3 → 7`
      : "v1+2*3 → 7",
  );
  const toolbarHint = computed(() =>
    inputScheme() === "Shuangpin" || inputScheme() === "Wubi86"
      ? `${inputScheme() === "Shuangpin" ? "双拼" : "五笔"}时不可用`
      : "打 v 再按空格：剪贴板、常用语、符号、表情",
  );
  const arrowKeys = signal(Config.get("ArrowKeys") || "Candidates");
  const pageKeysOpen = signal(false);
  const tipsOpen = signal(false);
  const arrowKeysHint = computed(() => {
    if (arrowKeys() === "Caret") return "↑ ↓ 选词，← → 在拼音里移动光标";
    return layout() === "Vertical" ? "↑ ↓ 选词，← → 翻页" : "← → 选词，↑ 上一页，↓ 展开更多候选";
  });
  const fcitxRunning = signal(Config.fcitxRunning());
  const fcitxStatus = computed(() => (fcitxRunning() ? "fcitx5 正在运行" : "fcitx5 没在运行"));
  const fcitxHint = computed(() =>
    fcitxRunning() ? "ArchType 已加载，改了设置会自动生效" : "在桌面设置里让 fcitx5 启动后再打字",
  );
  const keysMinus = signal(boolOf(Config.get("PageKeysMinusEqual"), true));
  const keysBracket = signal(boolOf(Config.get("PageKeysBracket"), false));
  const keysComma = signal(boolOf(Config.get("PageKeysCommaPeriod"), false));
  const keysTab = signal(boolOf(Config.get("PageKeysTab"), false));

  const setShiftFree = (on: boolean) => {
    const result = on ? Config.shiftFreeOn() : Config.shiftFreeOff();
    if (result === "memory") {
      shiftFreeOnState.set(on);
      return;
    }
    shiftFreeOnState.set(on);
    shiftFreed.set(Config.shiftFreeState());
    shiftConflicts.set(Config.shiftConflicts());
    if (result !== "ok" && result !== "没有要让出的 Shift" && result !== "没有记录要加回的 Shift") {
      shiftFreeOnState.set(!on);
    }
  };

  const wanxiangPath = signal(Config.lexiconWanxiangPath());
  const lexiconRaw = signal(Config.lexiconState());
  const lexiconBusyState = signal(Config.lexiconDownloadState());
  const lexiconProgress = signal(Config.lexiconDownloadProgress());
  const lexiconData = computed(() => {
    try {
      const value: unknown = JSON.parse(lexiconRaw());
      return typeof value === "object" && value !== null
        ? (value as { kind?: string; path?: string; wanxiangVersion?: string; latestVersion?: string })
        : {};
    } catch {
      return {};
    }
  });
  const lexiconKind = computed(() => lexiconData().kind ?? "rime-ice");
  const lexiconWanxiangVersion = computed(() => lexiconData().wanxiangVersion ?? "");
  const lexiconLatestVersion = computed(() => lexiconData().latestVersion ?? "");
  const lexiconCustomPath = computed(() => lexiconData().path ?? "");
  const lexiconBusy = computed(
    () =>
      lexiconBusyState().startsWith("downloading") || lexiconBusyState().startsWith("installing"),
  );
  const lexiconSubtitle = computed(() => {
    const state = lexiconBusyState();
    const [doneText, totalText] = lexiconProgress().split("/");
    const done = Number(doneText) || 0;
    const total = Number(totalText) || 0;
    if (state === "downloading" && total > 0) {
      return `正在下载：${humanSize(done)} / ${humanSize(total)}（${Math.floor((done * 100) / total)}%）`;
    }
    if (state === "downloading") return "正在连接…";
    if (state === "installing") return "正在安装…";
    if (state.startsWith("done:")) return `装好了 ${state.slice(5)}，已切过去，几秒后生效`;
    if (state.startsWith("error:")) return `没装好：${state.slice(6)}`;
    const version = lexiconWanxiangVersion();
    if (!version) return "词更多，要下载约 20 MB";
    return lexiconLatestVersion() && versionNewer(lexiconLatestVersion(), version)
      ? `已下载 ${version}，有新版 ${lexiconLatestVersion()}`
      : `已下载 ${version}`;
  });

  const refreshLexicon = () => {
    lexiconRaw.set(Config.lexiconState());
    lexiconBusyState.set(Config.lexiconDownloadState());
    lexiconProgress.set(Config.lexiconDownloadProgress());
  };

  const useLexicon = (path: string) => {
    Config.lexiconUse(path);
    refreshLexicon();
  };

  const cancelLexicon = () => {
    Config.lexiconDownloadCancel();
  };

  const downloadLexicon = () => {
    lexiconBusyState.set("downloading");
    Config.lexiconDownloadStart();
  };

  const deleteLexicon = () => {
    Config.lexiconDelete();
    refreshLexicon();
  };

  const clearStats = () => {
    if (clearState() !== "确认清除") {
      clearState.set("确认清除");
      return;
    }
    Config.statsClear();
    statsRaw.set(Config.stats());
    clearState.set("");
  };

  const statsTabs = [
    { key: "keys", title: "按键" },
    { key: "trend", title: "趋势" },
    { key: "types", title: "类型" },
    { key: "modes", title: "模式" },
    { key: "schemes", title: "方案" },
    { key: "candidates", title: "候选" },
  ];

  const applyProvider = (value: string) => {
    translateProvider.set(Number(value));
    const provider = translateProviders[Number(value)];
    if (provider.base) {
      translateBase.set(provider.base);
      Config.set("TranslateBaseUrl", provider.base);
    }
    if (provider.model) {
      translateModel.set(provider.model);
      Config.set("TranslateModel", provider.model);
    }
  };

  const runTranslateTest = () => {
    testState.set("测试中…");
    const result: { ok: boolean; text?: string; error?: string } = JSON.parse(
      Config.translateTest(
        translateBase(),
        translateKey(),
        translateModel(),
        translateTarget(),
        translateFormal(),
      ),
    );
    testState.set(result.ok ? `成功：${result.text ?? ""}` : `失败：${result.error ?? "未知错误"}`);
  };

  const onCustomWordChange = (value: string) => {
    customWord.set(value);
    if (!customPinyinEdited()) {
      customPinyin.set(value.trim() === "" ? "" : Config.pinyinGuessWord(value.trim()));
    }
  };

  const onCustomPinyinChange = (value: string) => {
    customPinyin.set(value);
    customPinyinEdited.set(true);
  };

  const exportCustomWords = () => {
    const path = Config.customWordsExport();
    customMessage.set(path ? `已导出到 ${path}` : "导出失败");
  };

  const importCustomWords = () => {
    const result = Config.customWordsImport();
    customMessage.set(result);
    customWords.set(Config.customWords());
  };

  const addCustomWord = () => {
    const added = Config.customWordAdd(customWord(), customPinyin());
    customMessage.set(added ? "已添加" : "词和编码都要填");
    if (added) {
      customWord.set("");
      customPinyin.set("");
      customPinyinEdited.set(false);
      customWords.set(Config.customWords());
    }
  };

  const removeCustomWord = (word: string) => {
    Config.customWordRemove(word);
    customWords.set(Config.customWords());
  };

  const downloadVoice = () => {
    voiceState.set("downloading");
    Config.voiceDownloadStart();
  };

  const downloadLocal = () => {
    localState.set("downloading");
    Config.localDownloadStart();
  };

  const cancelLocal = () => {
    Config.localDownloadCancel();
  };

  const deleteLocal = () => {
    Config.localDelete();
    localMissing.set(Config.localMissing());
    localState.set("idle");
  };

  const cancelVoice = () => {
    Config.voiceDownloadCancel();
  };

  const deleteVoice = () => {
    Config.voiceModelDelete();
    voiceModel.set(Config.voiceModel());
    voiceState.set("idle");
  };

  const runCheckUpdate = () => {
    updateState.set("检查中…");
    const result: { ok: boolean; tag?: string; newer?: boolean; url?: string; notes?: string; error?: string } =
      JSON.parse(Config.checkUpdate());
    if (!result.ok) {
      updateState.set(`失败：${result.error ?? "未知错误"}`);
      return;
    }
    updateTag.set(result.newer ? (result.tag ?? "") : "");
    updateState.set(result.newer ? "可以下载安装" : "已是最新");
    updateUrl.set(result.url ?? "");
    updateNotes.set(result.notes ?? "");
  };

  const save = (key: string, value: string) => {
    Config.set(key, value);
    Config.restartFcitx();
  };

  const applyTheme = (value: string) => {
    theme.set(value);
    Config.stateSet("theme", value);
    setColorScheme(value === "light" ? "light" : value === "dark" ? "dark" : "system");
  };

  const applyBlue = (on: boolean) => {
    blue.set(on);
    Config.classicuiSet("Theme", on ? "archtype" : "default");
    Config.classicuiSet("DarkTheme", on ? "archtype-dark" : "default-dark");
    Config.restartFcitx();
  };

  return (
    <View style={{ flex: 1, flexDirection: "row", backgroundColor: ui().page }}>
      <View
        style={{
          width: 210,
          backgroundColor: ui().sidebar,
          paddingLeft: 12,
          paddingRight: 12,
          paddingTop: 14,
          paddingBottom: 14,
          gap: 2,
        }}
      >
        <View style={{ flexDirection: "row", alignItems: "center", gap: 8, marginBottom: 14 }}>
          <View
            style={{
              width: 26,
              height: 26,
              borderRadius: 13,
              backgroundColor: ui().accent,
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Text style={{ color: "white", fontWeight: "800", fontSize: 14 }}>A</Text>
          </View>
          <Text style={{ color: ui().text, fontWeight: "800", fontSize: 16 }}>ArchType</Text>
          <Text style={{ color: ui().muted, fontSize: 12 }}>设置</Text>
        </View>
        {pages.map((p, i) => (
          <View
            onPress={() => current.set(i)}
            style={{
              paddingTop: 8,
              paddingBottom: 8,
              paddingLeft: 12,
              paddingRight: 12,
              borderRadius: 8,
              backgroundColor: current() === i ? ui().accent : "transparent",
            }}
          >
            <Text style={{ color: current() === i ? "white" : ui().text, fontSize: 14 }}>{p}</Text>
          </View>
        ))}
      </View>

      <Scroll style={{ flex: 1 }}>
        <View style={{ padding: 28, gap: 16 }}>
          <Text style={{ color: ui().text, fontWeight: "800", fontSize: 24 }}>{pages[current()]}</Text>

          {current() === 0 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>输入法</Text>
              <Card>
                <Row title={fcitxStatus()} subtitle={fcitxHint()}>
                  <SmallButton title="重启输入法" onPress={() => Config.restartFcitx()} />
                </Row>
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>候选词</Text>
              <Card>
                <Row title="排列方向" subtitle="横排 / 竖排">
                  <SelectBox
                    items={layoutItems}
                    value={layout()}
                    onChange={(value) => {
                      layout.set(value);
                      save("CandidateLayout", value);
                    }}
                  />
                </Row>
                <Row title="方向键" subtitle={arrowKeysHint()}>
                  <SelectBox
                    items={arrowKeyItems}
                    value={arrowKeys()}
                    onChange={(value) => {
                      arrowKeys.set(value);
                      save("ArrowKeys", value);
                    }}
                  />
                </Row>
                <Row title="每页个数" subtitle="3–9">
                  <StepInput
                    value={pageSize()}
                    min={3}
                    max={9}
                    onChange={(value) => {
                      pageSize.set(value);
                      save("PageSize", String(value));
                    }}
                  />
                </Row>
                <Row title="翻页键" subtitle="除了 Page Up / Page Down">
                  <SmallButton
                    title={pageKeysOpen() ? "收起" : "展开"}
                    onPress={() => pageKeysOpen.set(!pageKeysOpen())}
                  />
                </Row>
                {pageKeysOpen() ? (
                  <>
                    <Row title="- 和 =">
                      <Toggle
                        checked={keysMinus()}
                        onChange={(on) => {
                          keysMinus.set(on);
                          save("PageKeysMinusEqual", on ? "True" : "False");
                        }}
                      />
                    </Row>
                    <Row title="[ 和 ]" subtitle="打字途中不再出【】">
                      <Toggle
                        checked={keysBracket()}
                        onChange={(on) => {
                          keysBracket.set(on);
                          save("PageKeysBracket", on ? "True" : "False");
                        }}
                      />
                    </Row>
                    <Row title=", 和 ." subtitle="打字途中要先选词再打标点">
                      <Toggle
                        checked={keysComma()}
                        onChange={(on) => {
                          keysComma.set(on);
                          save("PageKeysCommaPeriod", on ? "True" : "False");
                        }}
                      />
                    </Row>
                    <Row title="Shift+Tab 和 Tab">
                      <Toggle
                        checked={keysTab()}
                        onChange={(on) => {
                          keysTab.set(on);
                          save("PageKeysTab", on ? "True" : "False");
                        }}
                      />
                    </Row>
                  </>
                ) : null}
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>外观</Text>
              <Card>
                <Row title="界面主题" subtitle="跟随系统 / 浅色 / 深色">
                  <SelectBox
                    items={themeItems}
                    value={theme()}
                    onChange={(value) => applyTheme(value)}
                  />
                </Row>
                <Row title="ArchType 蓝色主题" subtitle="选中的词是 ArchType 蓝，深色模式自动换深色版">
                  <Toggle checked={blue()} onChange={(on) => applyBlue(on)} />
                </Row>
                <Row title="候选字号" subtitle="8–32 磅，改了马上生效">
                  <StepInput
                    value={fontSize()}
                    min={8}
                    max={32}
                    onChange={(value) => {
                      fontSize.set(value);
                      Config.setFontSize(String(value));
                      Config.restartFcitx();
                    }}
                  />
                </Row>
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>帮助</Text>
              <Card>
                <Row title="小技巧" subtitle="点开看几个常用操作">
                  <SmallButton
                    title={tipsOpen() ? "收起" : "展开"}
                    onPress={() => tipsOpen.set(!tipsOpen())}
                  />
                </Row>
                {tipsOpen()
                  ? tips.map((tip) => <Row title={tip[0]} subtitle={tip[1]} />)
                  : null}
                <Row title="提交问题" subtitle="在 GitHub 上报告问题">
                  <SmallButton title="打开" onPress={() => Config.openUrl(issuesUrl)} />
                </Row>
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>
                配置文件：{Config.configPath()}
              </Text>
            </>
          ) : current() === 1 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>词库</Text>
              <Card>
                <View onPress={() => { if (lexiconKind() !== "rime-ice" && !lexiconBusy()) useLexicon(""); }}>
                  <Row title="雾凇拼音（默认）" subtitle="在安装包里，不用下载">
                    <RadioDot selected={lexiconKind() === "rime-ice"} />
                  </Row>
                </View>
                <View
                  onPress={() => {
                    if (lexiconKind() === "wanxiang" || lexiconBusy()) return;
                    if (lexiconWanxiangVersion()) useLexicon(wanxiangPath());
                    else downloadLexicon();
                  }}
                >
                  <Row title="万象拼音" subtitle={lexiconSubtitle()}>
                    <RadioDot selected={lexiconKind() === "wanxiang"} />
                  </Row>
                </View>
                <View style={{ flexDirection: "row", justifyContent: "flex-end", gap: 8, paddingBottom: 10 }}>
                  {lexiconBusy() ? (
                    <SmallButton title="取消" onPress={cancelLexicon} />
                  ) : lexiconWanxiangVersion() && lexiconLatestVersion() && versionNewer(lexiconLatestVersion(), lexiconWanxiangVersion()) ? (
                    <SmallButton title="更新" tone="accent" onPress={downloadLexicon} />
                  ) : !lexiconWanxiangVersion() ? (
                    <SmallButton title="下载万象" tone="accent" onPress={downloadLexicon} />
                  ) : null}
                  {lexiconWanxiangVersion() && lexiconKind() !== "wanxiang" && !lexiconBusy() ? (
                    <SmallButton title="删除万象" onPress={deleteLexicon} />
                  ) : null}
                </View>
                {lexiconKind() === "custom" ? (
                  <Row title="自定义词库" subtitle={lexiconCustomPath()}>
                    <RadioDot selected />
                  </Row>
                ) : null}
                <Text style={{ color: ui().muted, fontSize: 11 }}>
                  万象拼音词更多，比雾凇多占约 300 MB 内存；从 amzxyz/rime_wanxiang（CC BY 4.0）转换，只用了它的词条。
                </Text>
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>输入方案</Text>
              <Card>
                <View
                  onPress={() => {
                    inputScheme.set("Quanpin");
                    save("InputScheme", "Quanpin");
                  }}
                >
                  <Row title="拼音输入" subtitle="全拼，也能只打声母">
                    <RadioDot selected={inputScheme() === "Quanpin"} />
                  </Row>
                </View>
                <View
                  onPress={() => {
                    inputScheme.set("Shuangpin");
                    save("InputScheme", "Shuangpin");
                  }}
                >
                  <Row title="双拼输入" subtitle="每个字打两个键">
                    <RadioDot selected={inputScheme() === "Shuangpin"} />
                  </Row>
                </View>
                {inputScheme() === "Shuangpin" ? (
                  <>
                    <Row title="双拼方案">
                      <SelectBox
                        items={shuangpinItems}
                        value={shuangpin()}
                        onChange={(value) => {
                          shuangpin.set(value);
                          save("ShuangpinScheme", value);
                        }}
                      />
                    </Row>
                    <Row title="编辑区显示打的键" subtitle="不翻成全拼，中英混打时看得出打了什么">
                      <Toggle
                        checked={shuangpinShowKeys()}
                        onChange={(on) => {
                          shuangpinShowKeys.set(on);
                          save("ShuangpinShowKeys", on ? "True" : "False");
                        }}
                      />
                    </Row>
                  </>
                ) : null}
                <View
                  onPress={() => {
                    inputScheme.set("Wubi86");
                    save("InputScheme", "Wubi86");
                  }}
                >
                  <Row title="五笔输入" subtitle="86 版，z 键临时打拼音">
                    <RadioDot selected={inputScheme() === "Wubi86"} />
                  </Row>
                </View>
                {inputScheme() === "Wubi86" ? (
                  <>
                    <Row title="四码唯一自动上屏" subtitle="打满四码、只有一个字词时直接上屏">
                      <Toggle
                        checked={wubiAutoCommit()}
                        onChange={(on) => {
                          wubiAutoCommit.set(on);
                          save("WubiAutoCommit", on ? "True" : "False");
                        }}
                      />
                    </Row>
                    <Row title="编码提示" subtitle="候选后面显示还要打的码">
                      <Toggle
                        checked={wubiHint()}
                        onChange={(on) => {
                          wubiHint.set(on);
                          save("WubiHint", on ? "True" : "False");
                        }}
                      />
                    </Row>
                  </>
                ) : null}
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>中英文切换</Text>
              <Card>
                <Row title="单按 Shift" subtitle="单按一下 Shift 切到英文，再按一下回到中文">
                  <SelectBox
                    items={shiftItems}
                    value={shiftToggle()}
                    onChange={(value) => {
                      shiftToggle.set(value);
                      save("ShiftToggle", value);
                    }}
                  />
                </Row>
                <Row title="切到 ArchType 时" subtitle="从别的输入法切过来、开机后第一次用时；换窗口不变">
                  <SelectBox
                    items={initialModeItems}
                    value={initialMode()}
                    onChange={(value) => {
                      initialMode.set(value);
                      save("InitialMode", value);
                    }}
                  />
                </Row>
                {shiftConflictVisible() ? (
                  <Row title="Shift 只给 ArchType 用" subtitle={shiftConflictText()}>
                    <Toggle
                      checked={shiftFreeOnState()}
                      onChange={(on) => setShiftFree(on)}
                    />
                  </Row>
                ) : null}
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>拼音</Text>
              <Card>
                <Row title="模糊音" subtitle="比如 n 和 l 不分">
                  <Toggle
                    checked={fuzzyOn()}
                    onChange={(on) => {
                      fuzzyOn.set(on);
                      save("Fuzzy", on ? "True" : "False");
                    }}
                  />
                </Row>
                {fuzzyOn()
                  ? fuzzyGroups.map((group) => (
                      <Row title={group.label}>
                        <Toggle
                          checked={fuzzy()[group.key] ?? group.default}
                          onChange={(on) => {
                            fuzzy.update((current) => ({ ...current, [group.key]: on }));
                            save(group.key, on ? "True" : "False");
                          }}
                        />
                      </Row>
                    ))
                  : null}
                <Row title="按错键自动纠正" subtitle="zhognguo → 中国（全拼时）">
                  <Toggle
                    checked={typo()}
                    onChange={(on) => {
                      typo.set(on);
                      save("TypoCorrection", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="繁体字">
                  <Toggle
                    checked={traditional()}
                    onChange={(on) => {
                      traditional.set(on);
                      save("Traditional", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="更多生僻字" subtitle="要装能显示它们的字体，比如 fonts-hanazono">
                  <Toggle
                    checked={rareChars()}
                    onChange={(on) => {
                      rareChars.set(on);
                      save("RareExtChars", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="标点样式" subtitle="数字后面的符号总是半角的，比如 3.14。">
                  <SelectBox
                    items={punctStyleItems}
                    value={punctStyle()}
                    onChange={(value) => {
                      punctStyle.set(value);
                      save("PunctuationStyle", value);
                    }}
                  />
                </Row>
                <Row title="中文标点自动配对" subtitle="打（【《「“‘ 时自动补上右半边">
                  <Toggle
                    checked={autoPair()}
                    onChange={(on) => {
                      autoPair.set(on);
                      save("AutoPairPunct", on ? "True" : "False");
                    }}
                  />
                </Row>
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>快捷输入</Text>
              <Card>
                <Row title="日期和时间" subtitle="打 riqi、shijian、xingqi（五笔前面加 z）">
                  <Toggle
                    checked={dateTime()}
                    onChange={(on) => {
                      dateTime.set(on);
                      save("DateTime", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="计算器" subtitle={calcHint()}>
                  <Toggle
                    checked={calculator()}
                    onChange={(on) => {
                      calculator.set(on);
                      save("Calculator", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="工具栏" subtitle={toolbarHint()}>
                  <Toggle
                    checked={toolbar() && inputScheme() === "Quanpin"}
                    onChange={(on) => {
                      toolbar.set(on);
                      save("Toolbar", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="记录剪贴板历史" subtitle="复制的密码也可能被记下">
                  <Toggle
                    checked={clipboardHistory()}
                    onChange={(on) => {
                      clipboardHistory.set(on);
                      save("ClipboardHistory", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="分号、引号选词" subtitle="用引号选词后，引号不能再用来隔开拼音（xi'an）。">
                  <SelectBox
                    items={selectKeyItems}
                    value={selectKeys()}
                    onChange={(value) => {
                      selectKeys.set(value);
                      save("SelectKeys", value);
                    }}
                  />
                </Row>
                <Row title="以词定字" subtitle="按 [ 取第一个字，按 ] 取最后一个字">
                  <Toggle
                    checked={pickChar()}
                    onChange={(on) => {
                      pickChar.set(on);
                      save("PickChar", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="联想" subtitle="上屏以后给出常跟在后面的词，按 Tab 选">
                  <Toggle
                    checked={association()}
                    onChange={(on) => {
                      association.set(on);
                      save("Association", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="中英文之间自动加空格" subtitle="第5章 → 第 5 章">
                  <Toggle
                    checked={autoSpace()}
                    onChange={(on) => {
                      autoSpace.set(on);
                      save("AutoSpace", on ? "True" : "False");
                    }}
                  />
                </Row>
                {autoSpace() ? (
                  <>
                    <Row title="汉字和英文字母之间">
                      <Toggle
                        checked={autoSpaceLetters()}
                        onChange={(on) => {
                          autoSpaceLetters.set(on);
                          save("AutoSpaceLetters", on ? "True" : "False");
                        }}
                      />
                    </Row>
                    <Row title="汉字和数字之间">
                      <Toggle
                        checked={autoSpaceDigits()}
                        onChange={(on) => {
                          autoSpaceDigits.set(on);
                          save("AutoSpaceDigits", on ? "True" : "False");
                        }}
                      />
                    </Row>
                  </>
                ) : null}
              </Card>

            </>
          ) : current() === 2 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>表情</Text>
              <Card>
                <Row title="打字时推荐 emoji" subtitle="打「开心」「点赞」时，候选里有对应的 emoji">
                  <Toggle
                    checked={emojiSuggest()}
                    onChange={(on) => {
                      emojiSuggest.set(on);
                      save("EmojiSuggest", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="表情面板快捷键" subtitle="按快捷键打开，打拼音或英文搜 emoji 和符号（xiao、pizza、youjiantou），方向键挑，空格或回车上屏，Esc 关闭。Super 就是 Windows 键。">
                  <SelectBox
                    items={emojiKeyItems}
                    value={emojiKey()}
                    onChange={(value) => {
                      emojiKey.set(value);
                      save("EmojiKey", value);
                    }}
                  />
                </Row>
              </Card>
            </>
          ) : current() === 6 ? (
            <>
              <Card>
                <Row title="记录打字统计" subtitle="只在本机按天记计数，不记打的字">
                  <Toggle
                    checked={typingStats()}
                    onChange={(on) => {
                      typingStats.set(on);
                      save("TypingStats", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="清除统计" subtitle="删掉已经记下的全部计数">
                  <SmallButton title={clearState() || "清除"} onPress={clearStats} />
                </Row>
              </Card>

              <Card>
                <View style={{ flexDirection: "row", gap: 16 }}>
                  <Metric
                    title="今日输入"
                    value={formatNumber(summary().todayChars)}
                    unit="字符"
                    note={
                      summary().bestDay
                        ? `单日最多 ${formatNumber(summary().bestChars)} 字符 · ${summary().bestDay}`
                        : "还没有记录"
                    }
                  />
                  <Metric
                    title="累计输入"
                    value={formatNumber(summary().totalChars)}
                    unit="字符"
                    note={`日均 ${formatNumber(
                      summary().recordedDays ? summary().totalChars / summary().recordedDays : 0,
                    )} 字符 · ${summary().recordedDays} 天有记录`}
                  />
                  <Metric
                    title="今日活跃"
                    value={formatActive(summary().todayActive)}
                    note={`累计 ${formatActive(summary().totalActive)}`}
                  />
                </View>
                <View style={{ flexDirection: "row", gap: 16 }}>
                  <Metric
                    title="今日速度"
                    value={`${summary().todaySpeed}`}
                    unit="字/分钟"
                    note={
                      summary().fastestDay
                        ? `单日最快 ${summary().fastestSpeed} 字/分钟 · ${summary().fastestDay}`
                        : ""
                    }
                  />
                  <Metric
                    title="平均速度"
                    value={`${summary().avgSpeed}`}
                    unit="字/分钟"
                    note="汉字与字母 ÷ 活跃时间"
                  />
                  <Metric
                    title="连续天数"
                    value={`${summary().streak}`}
                    unit="天"
                    note={`最长 ${summary().longest} 天`}
                  />
                </View>
              </Card>

              <View
                style={{
                  flexDirection: "row",
                  backgroundColor: ui().sidebar,
                  borderRadius: 8,
                  padding: 3,
                  gap: 2,
                }}
              >
                {statsTabs.map((tab) => (
                  <View
                    onPress={() => statsTab.set(tab.key)}
                    style={{
                      flex: 1,
                      alignItems: "center",
                      paddingTop: 6,
                      paddingBottom: 6,
                      borderRadius: 6,
                      backgroundColor: statsTab() === tab.key ? ui().card : "transparent",
                    }}
                  >
                    <Text
                      style={{
                        color: statsTab() === tab.key ? ui().text : ui().muted,
                        fontSize: 13,
                      }}
                    >
                      {tab.title}
                    </Text>
                  </View>
                ))}
              </View>

              {statsTab() === "keys" ? (
                <>
                  <Card>
                    <Text style={{ color: ui().text, fontSize: 14, fontWeight: "800" }}>
                      按键热力图 · 累计
                    </Text>
                    <Text style={{ color: ui().muted, fontSize: 12 }}>
                      共 {formatNumber(summary().totalKeys)} 次按键，颜色越深按得越多
                    </Text>
                    <View style={{ alignItems: "center", gap: 4, paddingTop: 12, paddingBottom: 8 }}>
                      {keyboardRows.map((row) => (
                        <View style={{ flexDirection: "row", gap: 4 }}>
                          {row.map((key) =>
                            key.code ? (
                              <KeyCap
                                label={key.label}
                                weight={key.weight}
                                count={summary().keys[key.code] ?? 0}
                                max={maxKeyCount(summary().keys)}
                              />
                            ) : (
                              <View style={{ width: key.weight * 36 - 4 }} />
                            ),
                          )}
                        </View>
                      ))}
                    </View>
                    <View style={{ flexDirection: "row", justifyContent: "center", alignItems: "center", gap: 6, marginTop: 10 }}>
                      <Text style={{ color: ui().muted, fontSize: 11 }}>少</Text>
                      {[0, 1, 2, 3, 4].map((level) => (
                        <View
                          style={{
                            width: 12,
                            height: 12,
                            borderRadius: 2,
                            backgroundColor: heatColor(level, 4),
                          }}
                        />
                      ))}
                      <Text style={{ color: ui().muted, fontSize: 11 }}>多</Text>
                    </View>
                  </Card>
                  <Card>
                    <Text style={{ color: ui().muted, fontSize: 12 }}>其他键</Text>
                    {Array.from({ length: Math.ceil(otherKeys(summary().keys).length / 5) }).map(
                      (_, rowIndex) => (
                        <View style={{ flexDirection: "row", gap: 6 }}>
                          {otherKeys(summary().keys)
                            .slice(rowIndex * 5, rowIndex * 5 + 5)
                            .map((key) => (
                              <KeyChip
                                label={key.label}
                                count={key.count}
                                max={maxKeyCount(summary().keys)}
                              />
                            ))}
                        </View>
                      ),
                    )}
                    <View style={{ height: 4 }} />
                    {Object.entries(summary().keys)
                      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
                      .slice(0, 5)
                      .map(([code, count], index) => (
                        <View
                          style={{ flexDirection: "row", gap: 8, paddingTop: 2, paddingBottom: 2 }}
                        >
                          <Text style={{ color: ui().muted, fontSize: 12, width: 18 }}>{index + 1}</Text>
                          <Text style={{ color: ui().text, fontSize: 12, flex: 1 }}>
                            {keyLabel(code)}
                          </Text>
                          <Text style={{ color: ui().text, fontSize: 12 }}>
                            {formatNumber(count)} 次
                          </Text>
                        </View>
                      ))}
                  </Card>
                </>
              ) : statsTab() === "trend" ? (
                <Card>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>每日趋势 · 近 30 天</Text>
                  {trendPeak() === 0 ? (
                    <Text style={{ color: ui().muted, fontSize: 12, paddingTop: 10 }}>
                      这 30 天还没有输入记录。
                    </Text>
                  ) : (
                    <>
                      <View
                        style={{
                          flexDirection: "row",
                          alignItems: "flex-end",
                          width: "100%",
                          height: 140,
                          gap: 3,
                          paddingTop: 14,
                        }}
                      >
                        {summary().recent.map((day) => (
                          <View
                            style={{
                              flex: 1,
                              height: Math.round((day.chars / Math.max(1, trendPeak())) * 140),
                              backgroundColor: ui().accent,
                              borderRadius: 2,
                            }}
                          />
                        ))}
                      </View>
                      <View style={{ height: 1, backgroundColor: ui().border, marginTop: 3 }} />
                      <View
                        style={{ flexDirection: "row", justifyContent: "space-between", paddingTop: 6 }}
                      >
                        <Text style={{ color: ui().muted, fontSize: 12 }}>
                          {summary().recent[0].day.slice(5)}
                        </Text>
                        <Text style={{ color: ui().muted, fontSize: 12 }}>
                          {summary().recent[summary().recent.length - 1].day.slice(5)}
                        </Text>
                      </View>
                      <Text style={{ color: ui().muted, fontSize: 12, paddingTop: 6 }}>
                        共 {formatNumber(trendTotal())} 字符 · 最高一天 {formatNumber(trendPeak())} 字符
                      </Text>
                    </>
                  )}
                </Card>
              ) : statsTab() === "types" ? (
                <Card>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>字符类型 · 累计</Text>
                  {countRows(summary().kinds, kindNames).map((row) => (
                    <Row title={row.label} subtitle={`${formatNumber(row.count)} 次`} />
                  ))}
                </Card>
              ) : statsTab() === "modes" ? (
                <Card>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>输入模式 · 累计</Text>
                  {countRows(summary().sources, sourceNames).map((row) => (
                    <Row title={row.label} subtitle={`${formatNumber(row.count)} 字符`} />
                  ))}
                </Card>
              ) : statsTab() === "schemes" ? (
                <Card>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>方案 · 累计</Text>
                  {countRows(summary().schemes, schemeNames).map((row) => (
                    <Row title={row.label} subtitle={`${formatNumber(row.count)} 字符`} />
                  ))}
                </Card>
              ) : (
                <>
                  <Card>
                    <Text style={{ color: ui().muted, fontSize: 12 }}>候选位置 · 累计</Text>
                    {summary().selections.map((count, index) => (
                      <Row title={`第 ${index + 1} 个候选`} subtitle={`${formatNumber(count)} 次`} />
                    ))}
                    <Row title="超过候选数" subtitle={`${formatNumber(summary().beyond)} 次`} />
                  </Card>
                  <Card>
                    <Text style={{ color: ui().muted, fontSize: 12 }}>候选位置 · 今日</Text>
                    {(summary().todaySelections.length > 0
                      ? summary().todaySelections
                      : [0, 0, 0, 0, 0, 0, 0, 0, 0]
                    ).map((count, index) => (
                      <Row title={`第 ${index + 1} 个候选`} subtitle={`${formatNumber(count)} 次`} />
                    ))}
                  </Card>
                </>
              )}

              <Text style={{ color: ui().muted, fontSize: 12 }}>
                速度统计汉字与字母；两次上屏间隔超过 10 秒算休息，不计入活跃时长。统计只在本机，清除后输入法会丢掉攒着的旧计数。
              </Text>
            </>
          ) : current() === 3 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>语音输入</Text>
              <Card>
                <Row title="语音输入" subtitle={voiceModel() ? "按住语音键说话，松开就上屏" : "先在下面下载语音模型"}>
                  <Toggle
                    checked={voiceOn() && voiceModel() !== ""}
                    disabled={voiceModel() === ""}
                    onChange={(on) => {
                      voiceOn.set(on);
                      save("Voice", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="语音键">
                  <SelectBox
                    items={voiceKeyItems}
                    value={voiceKey()}
                    onChange={(value) => {
                      voiceKey.set(value);
                      save("VoiceKey", value);
                    }}
                  />
                </Row>
                <Row title="同时复制到剪贴板" subtitle="要装 wl-clipboard 或 xclip">
                  <Toggle
                    checked={voiceCopy()}
                    onChange={(on) => {
                      voiceCopy.set(on);
                      save("VoiceCopy", on ? "True" : "False");
                    }}
                  />
                </Row>
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>识别模型</Text>
              <Card>
                <Row title="语音模型" subtitle={voiceModelSubtitle()}>
                  <SmallButton
                    title={voiceBusy() ? "取消" : voiceModel() ? "删除" : "下载"}
                    tone={voiceModel() || voiceBusy() ? "plain" : "accent"}
                    onPress={voiceBusy() ? cancelVoice : voiceModel() ? deleteVoice : downloadVoice}
                  />
                </Row>
              </Card>
            </>
          ) : current() === 4 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>互译</Text>
              <Card>
                <Row title="互译" subtitle="单按 Shift 切到「译」，照常打中文，停一下就翻成外文。回车用译文，Shift+回车用原文，Esc 取消。">
                  <Toggle
                    checked={translateOn()}
                    onChange={(on) => {
                      translateOn.set(on);
                      save("Translate", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="翻译成" subtitle="中文翻成这种语言；这种语言（和别的外文）翻成中文。">
                  <SelectBox
                    items={translateTargetItems}
                    value={translateTarget()}
                    onChange={(value) => {
                      translateTarget.set(value);
                      save("TranslateTarget", value);
                    }}
                  />
                </Row>
                <Row title="正式语气" subtitle="默认是聊天的口吻">
                  <Toggle
                    checked={translateFormal()}
                    onChange={(on) => {
                      translateFormal.set(on);
                      save("TranslateFormal", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="翻译引擎" subtitle={translateEngineNotes[engineOf(translateLocal(), translateBase())]}>
                  <SelectBox
                    items={translateEngineItems}
                    value={engineOf(translateLocal(), translateBase())}
                    onChange={(value) => {
                      if (value === "local") {
                        translateLocal.set(true);
                        Config.set("TranslateLocal", "True");
                      } else if (value === "bilibili") {
                        translateLocal.set(false);
                        translateBase.set(BILIBILI_URL);
                        Config.set("TranslateLocal", "False");
                        Config.set("TranslateBaseUrl", BILIBILI_URL);
                      } else {
                        translateLocal.set(false);
                        Config.set("TranslateLocal", "False");
                      }
                      Config.restartFcitx();
                    }}
                  />
                </Row>
              </Card>

              {engineOf(translateLocal(), translateBase()) !== "local" ? (
                <>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>翻译服务</Text>
                  <Card>
                    {engineOf(translateLocal(), translateBase()) === "custom" ? (
                      <>
                        <Row title="服务商" subtitle="选一家，自动填地址和建议的模型">
                          <SelectBox
                            items={providerItems}
                            value={String(translateProvider())}
                            onChange={applyProvider}
                          />
                        </Row>
                        <Row title="接口地址" subtitle="OpenAI 格式，最后不用带 /chat/completions">
                          <InputBox
                            value={translateBase()}
                            placeholder="https://api.example.com/v1"
                            onChange={(value) => {
                              translateBase.set(value);
                              Config.set("TranslateBaseUrl", value);
                            }}
                          />
                        </Row>
                        <Row title="API Key" subtitle="只存在这台电脑上；本机服务可以不填">
                          <InputBox
                            value={translateKey()}
                            masked
                            onChange={(value) => {
                              translateKey.set(value);
                              Config.setTranslateKey(value);
                            }}
                          />
                        </Row>
                        <Row title="模型" subtitle="留空用服务商的默认模型">
                          <InputBox
                            value={translateModel()}
                            placeholder="deepseek-chat"
                            onChange={(value) => {
                              translateModel.set(value);
                              Config.set("TranslateModel", value);
                            }}
                          />
                        </Row>
                      </>
                    ) : null}
                    <Row title="测试连接" subtitle={testState() || "用「你好」试一下"}>
                      <SmallButton title="测试" onPress={runTranslateTest} />
                    </Row>
                  </Card>
                </>
              ) : null}

              {engineOf(translateLocal(), translateBase()) === "bilibili" ? (
                <>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>哔哩哔哩 Index-Translate</Text>
                  <Card>
                    <Row title="模型" subtitle="2B 最快；9B 译得更好一点，稍慢；35B-A3B 是预览版">
                      <SelectBox
                        items={biliModelItems}
                        value={translateModel() || "Index-Translate-2B"}
                        onChange={(value) => {
                          translateModel.set(value);
                          Config.set("TranslateModel", value);
                        }}
                      />
                    </Row>
                    <Row title="测试连接" subtitle={testState() || "用「你好」试一下"}>
                      <SmallButton title="测试" onPress={runTranslateTest} />
                    </Row>
                  </Card>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>
                    要翻译的文字会发到哔哩哔哩的服务器（index-translate.bilibili.com）
                  </Text>
                </>
              ) : null}

              {engineOf(translateLocal(), translateBase()) === "local" ? (
                <>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>离线翻译模型</Text>
                  <Card>
                    <Row title="翻译模型" subtitle={localSubtitle()}>
                      <SmallButton
                        title={localBusy() ? "取消" : localMissing() === "ready" ? "删除" : "下载"}
                        tone={localMissing() === "ready" || localBusy() ? "plain" : "accent"}
                        onPress={
                          localBusy()
                            ? cancelLocal
                            : localMissing() === "ready"
                              ? deleteLocal
                              : downloadLocal
                        }
                      />
                    </Row>
                  </Card>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>
                    模型属于腾讯，用的是《腾讯混元社区许可协议》：不适用于欧盟、英国和韩国境内，月活超过 1 亿的产品要另外向腾讯申请。模型从 Hugging Face 下载，ArchType 不分发它。
                  </Text>
                </>
              ) : null}
            </>
          ) : current() === 5 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>自定义词</Text>
              <Card>
                <Row title="词" subtitle="编码会按词自动填好，多音字请检查">
                  <InputBox
                    value={customWord()}
                    placeholder="例如：ArchType"
                    onChange={onCustomWordChange}
                  />
                </Row>
                <Row title="编码" subtitle="拼音（空格分开音节），也可以是 mail 这类字母">
                  <InputBox
                    value={customPinyin()}
                    placeholder="例如：a r c h"
                    onChange={onCustomPinyinChange}
                  />
                </Row>
                <Row title="添加" subtitle={customMessage()}>
                  <SmallButton title="添加" tone="accent" onPress={addCustomWord} />
                </Row>
              </Card>
              <Card>
                <Row title="搜索" subtitle="按词或编码过滤">
                  <InputBox
                    value={customSearch()}
                    placeholder="输入词或编码"
                    onChange={(value) => customSearch.set(value)}
                  />
                </Row>
                <Row title="导入导出" subtitle="~/Downloads/archtype-custom-words.tsv">
                  <SmallButton title="导出" onPress={exportCustomWords} />
                  <SmallButton title="导入" onPress={importCustomWords} />
                </Row>
              </Card>
              <Card>
                {customWords().trim() ? (
                  (() => {
                    const query = customSearch().trim().toLowerCase();
                    const rows = customWords()
                      .split("\n")
                      .filter((line) => line.trim() !== "" && !line.trimStart().startsWith("#"))
                      .filter((line) => {
                        if (query === "") return true;
                        const parts = line.split("\t");
                        return (parts[1] ?? "").toLowerCase().includes(query)
                          || (parts[0] ?? "").toLowerCase().includes(query);
                      });
                    return rows.length > 0 ? (
                      rows.map((line) => {
                        const parts = line.split("\t");
                        const word = parts[1] ?? parts[0];
                        return (
                          <Row title={word} subtitle={`编码 ${parts[0] ?? ""}`}>
                            <SmallButton title="删除" onPress={() => removeCustomWord(word)} />
                          </Row>
                        );
                      })
                    ) : (
                      <Row title="没找到" subtitle="换个词或编码再搜" />
                    );
                  })()
                ) : (
                  <Row title="还没有自定义词" subtitle="在上面的卡片里添加" />
                )}
              </Card>
            </>
          ) : current() === 7 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>版本</Text>
              <Card>
                <Row title={`当前版本 ${Config.version()}`} subtitle={updateSource}>
                  <SmallButton title="检查更新" onPress={runCheckUpdate} />
                </Row>
                <Row title={updateTitle()} subtitle={updateState()}>
                  {updateUrl() ? (
                    <SmallButton title="打开下载页" onPress={() => Config.openUrl(updateUrl())} />
                  ) : null}
                </Row>
              </Card>

              {updateNotes() ? (
                <>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>更新内容</Text>
                  <Card>
                    <Text style={{ color: ui().text, fontSize: 13 }}>{updateNotes()}</Text>
                  </Card>
                </>
              ) : null}

              <Text style={{ color: ui().muted, fontSize: 12 }}>自动</Text>
              <Card>
                <Row title="自动检查更新" subtitle="有新版本时发通知">
                  <Toggle
                    checked={checkUpdate()}
                    onChange={(on) => {
                      checkUpdate.set(on);
                      save("CheckUpdate", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="Beta 计划" subtitle="也接收预发布版的更新">
                  <Toggle
                    checked={betaUpdates()}
                    onChange={(on) => {
                      betaUpdates.set(on);
                      save("BetaUpdates", on ? "True" : "False");
                    }}
                  />
                </Row>
              </Card>
            </>
          ) : (
            <Card>
              <Row title={pages[current()]} subtitle="这一页的设置在逐步迁移中" />
            </Card>
          )}

          <Text style={{ color: ui().muted, fontSize: 12 }}>
            ArchType 设置（NSH 预览版，版本 {Config.version()}）
          </Text>
        </View>
      </Scroll>
    </View>
  );
}

// 启动时就应用保存的主题:否则 colorScheme 还是系统的,配置里的浅色要手动切一次才对。
{
  const saved = Config.stateGet("theme") || "system";
  setColorScheme(saved === "light" ? "light" : saved === "dark" ? "dark" : "system");
}

mount(App);
