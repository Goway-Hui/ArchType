import { mount, setColorScheme, colorScheme, computed, signal, Scroll, Text, View } from "nsh";
import { Button, Input, NumberInput, Select, Switch } from "nsh/base";
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

function maxRecent(summary: StatsSummary): number {
  return Math.max(1, ...summary.recent.map((day) => day.chars));
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
        width: props.weight * 30 - 4,
        height: 30,
        borderRadius: 6,
        borderWidth: 1,
        borderColor: ui().border,
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
        flex: 1,
        flexDirection: "row",
        alignItems: "center",
        gap: 6,
        borderWidth: 1,
        borderColor: ui().border,
        borderRadius: 6,
        paddingLeft: 8,
        paddingRight: 8,
        paddingTop: 5,
        paddingBottom: 5,
        minWidth: 150,
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
      <Text style={{ color: ui().text, fontSize: 12, flex: 1 }}>{props.label}</Text>
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
        paddingLeft: 16,
        paddingRight: 16,
        paddingTop: 2,
        paddingBottom: 2,
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

function Toggle(props: { checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <Switch
      checked={props.checked}
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
  { value: "bilibili", title: "哔哩哔哩（免费）" },
  { value: "custom", title: "自定义翻译服务" },
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

const fuzzyGroups = [
  { key: "FuzzyZh", label: "z = zh" },
  { key: "FuzzyCh", label: "c = ch" },
  { key: "FuzzySh", label: "s = sh" },
  { key: "FuzzyNL", label: "n = l" },
  { key: "FuzzyFH", label: "f = h" },
  { key: "FuzzyRL", label: "r = l" },
  { key: "FuzzyAn", label: "an = ang" },
  { key: "FuzzyAnAi", label: "an = ai" },
  { key: "FuzzyIan", label: "ian = iang" },
  { key: "FuzzyUan", label: "uan = uang" },
  { key: "FuzzyHuangWang", label: "huang = wang" },
  { key: "FuzzyEn", label: "en = eng" },
  { key: "FuzzyEngOng", label: "eng = ong" },
  { key: "FuzzyIn", label: "in = ing" },
  { key: "FuzzyUn", label: "un = ong" },
  { key: "FuzzyUnIong", label: "un = iong" },
];

const inputSchemeItems = [
  { value: "Quanpin", title: "全拼" },
  { value: "Shuangpin", title: "双拼" },
  { value: "Wubi86", title: "五笔 86" },
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
  { value: "Left", title: "只左 Shift" },
  { value: "Right", title: "只右 Shift" },
  { value: "Off", title: "关掉" },
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
  for (let offset = 13; offset >= 0; offset -= 1) {
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
  const autoPair = signal(boolOf(Config.get("AutoPairPunct"), false));
  const fuzzy = signal<Record<string, boolean>>(
    Object.fromEntries(fuzzyGroups.map((group) => [group.key, boolOf(Config.get(group.key), false)])),
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
  const voiceState = signal(Config.voiceDownloadState());
  const voiceBusy = computed(
    () =>
      voiceState().startsWith("downloading") ||
      voiceState().startsWith("verifying") ||
      voiceState().startsWith("extracting"),
  );
  const updateState = signal("");
  const updateUrl = signal("");

  if (!voicePollStarted) {
    voicePollStarted = true;
    setInterval(() => {
      const state = Config.voiceDownloadState();
      voiceState.set(state);
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

  const keysMinus = signal(boolOf(Config.get("PageKeysMinusEqual"), true));
  const keysBracket = signal(boolOf(Config.get("PageKeysBracket"), false));
  const keysComma = signal(boolOf(Config.get("PageKeysCommaPeriod"), false));
  const keysTab = signal(boolOf(Config.get("PageKeysTab"), false));

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

  const addCustomWord = () => {
    const added = Config.customWordAdd(customWord(), customPinyin());
    customMessage.set(added ? "已添加" : "词和编码都要填");
    if (added) {
      customWord.set("");
      customPinyin.set("");
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

  const deleteVoice = () => {
    Config.voiceModelDelete();
    voiceModel.set(Config.voiceModel());
    voiceState.set("idle");
  };

  const runCheckUpdate = () => {
    updateState.set("检查中…");
    const result: { ok: boolean; tag?: string; newer?: boolean; url?: string; error?: string } =
      JSON.parse(Config.checkUpdate());
    if (!result.ok) {
      updateState.set(`失败：${result.error ?? "未知错误"}`);
      return;
    }
    updateState.set(result.newer ? `有新版 ${result.tag}` : "已是最新");
    updateUrl.set(result.url ?? "");
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
              <Text style={{ color: ui().muted, fontSize: 12 }}>候选词</Text>
              <Card>
                <Row title="候选排列" subtitle="横排 / 竖排">
                  <SelectBox
                    items={layoutItems}
                    value={layout()}
                    onChange={(value) => {
                      layout.set(value);
                      save("CandidateLayout", value);
                    }}
                  />
                </Row>
                <Row title="每页个数" subtitle="3–9">
                  <NumberInput
                    value={pageSize()}
                    min={3}
                    max={9}
                    step={1}
                    onChange={(e) => {
                      const value = e.value ?? 9;
                      pageSize.set(value);
                      save("PageSize", String(value));
                    }}
                  />
                </Row>
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>翻页键</Text>
              <Card>
                <Row title="- 和 =" subtitle="翻到头时不当标点上屏">
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
                  <NumberInput
                    value={fontSize()}
                    min={8}
                    max={32}
                    step={1}
                    onChange={(e) => {
                      const value = e.value ?? 10;
                      fontSize.set(value);
                      Config.setFontSize(String(value));
                      Config.restartFcitx();
                    }}
                  />
                </Row>
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>
                配置文件：{Config.configPath()}
              </Text>
            </>
          ) : current() === 1 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>输入方案</Text>
              <Card>
                <Row title="输入方案" subtitle="全拼 / 双拼 / 五笔">
                  <SelectBox
                    items={inputSchemeItems}
                    value={inputScheme()}
                    onChange={(value) => {
                      inputScheme.set(value);
                      save("InputScheme", value);
                    }}
                  />
                </Row>
                {inputScheme() === "Shuangpin" ? (
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
                ) : null}
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>模糊音</Text>
              <Card>
                {fuzzyGroups.map((group) => (
                  <Row title={group.label}>
                    <Toggle
                      checked={fuzzy()[group.key] ?? false}
                      onChange={(on) => {
                        fuzzy.update((current) => ({ ...current, [group.key]: on }));
                        save(group.key, on ? "True" : "False");
                      }}
                    />
                  </Row>
                ))}
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>其他</Text>
              <Card>
                <Row title="按错键自动纠正" subtitle="比如 weixni 打成微信">
                  <Toggle
                    checked={typo()}
                    onChange={(on) => {
                      typo.set(on);
                      save("TypoCorrection", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="单按 Shift 切换中英文">
                  <SelectBox
                    items={shiftItems}
                    value={shiftToggle()}
                    onChange={(value) => {
                      shiftToggle.set(value);
                      save("ShiftToggle", value);
                    }}
                  />
                </Row>
                <Row title="中文标点自动配对" subtitle="打 ( 出（），光标在中间">
                  <Toggle
                    checked={autoPair()}
                    onChange={(on) => {
                      autoPair.set(on);
                      save("AutoPairPunct", on ? "True" : "False");
                    }}
                  />
                </Row>
              </Card>

              <Text style={{ color: ui().muted, fontSize: 12 }}>
                词库：{Config.get("Lexicon") || "雾凇拼音（默认）"}
              </Text>
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
                <Row title="表情面板快捷键" subtitle="打拼音或英文搜 emoji 和符号">
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
                    <Text style={{ color: ui().muted, fontSize: 12 }}>
                      按键热力图 · 累计
                      共 {formatNumber(summary().totalKeys)} 次按键，颜色越深按得越多
                    </Text>
                    <View style={{ alignItems: "center", gap: 4 }}>
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
                              <View style={{ width: key.weight * 30 }} />
                            ),
                          )}
                        </View>
                      ))}
                    </View>
                    <View style={{ flexDirection: "row", justifyContent: "center", alignItems: "center", gap: 6, marginTop: 8 }}>
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
                  <Text style={{ color: ui().muted, fontSize: 12 }}>最近 14 天</Text>
                  {summary().recent.map((day) => (
                    <View
                      style={{
                        flexDirection: "row",
                        alignItems: "center",
                        gap: 10,
                        paddingTop: 3,
                        paddingBottom: 3,
                      }}
                    >
                      <Text style={{ color: ui().muted, fontSize: 12, width: 70 }}>{day.day.slice(5)}</Text>
                      <View style={{ flex: 1, height: 8, borderRadius: 4, backgroundColor: ui().page }}>
                        <View
                          style={{
                            width: `${Math.round((day.chars / maxRecent(summary())) * 100)}%`,
                            height: 8,
                            borderRadius: 4,
                            backgroundColor: ui().accent,
                          }}
                        />
                      </View>
                      <Text
                        style={{
                          color: ui().text,
                          fontSize: 12,
                          width: 60,
                          textAlign: "right",
                        }}
                      >
                        {formatNumber(day.chars)}
                      </Text>
                    </View>
                  ))}
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
              <Text style={{ color: ui().muted, fontSize: 12 }}>语音</Text>
              <Card>
                <Row title="语音输入" subtitle="按住语音键说话，松开上屏">
                  <Toggle
                    checked={voiceOn()}
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
                <Row title="识别结果同时复制到剪贴板">
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
                <Row title="模型状态" subtitle={voiceModel() ? `已安装：${voiceModel()}` : "未下载"}>
                  <SmallButton
                    title={voiceModel() ? "删除模型" : "下载模型"}
                    onPress={voiceModel() ? deleteVoice : downloadVoice}
                    disabled={voiceBusy()}
                  />
                </Row>
                <Row title="下载状态" subtitle={voiceState()} />
              </Card>
            </>
          ) : current() === 4 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>翻译</Text>
              <Card>
                <Row title="互译" subtitle="按互译键进入「译」模式，打字后上屏译文">
                  <Toggle
                    checked={translateOn()}
                    onChange={(on) => {
                      translateOn.set(on);
                      save("Translate", on ? "True" : "False");
                    }}
                  />
                </Row>
                <Row title="翻译引擎">
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
                <Row title="目标语言">
                  <SelectBox
                    items={translateTargetItems}
                    value={translateTarget()}
                    onChange={(value) => {
                      translateTarget.set(value);
                      save("TranslateTarget", value);
                    }}
                  />
                </Row>
              </Card>
              {engineOf(translateLocal(), translateBase()) === "custom" ? (
                <>
                  <Text style={{ color: ui().muted, fontSize: 12 }}>翻译服务</Text>
                  <Card>
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
                    <Row title="正式语气" subtitle="默认是聊天的口吻">
                      <Toggle
                        checked={translateFormal()}
                        onChange={(on) => {
                          translateFormal.set(on);
                          save("TranslateFormal", on ? "True" : "False");
                        }}
                      />
                    </Row>
                    <Row title="测试连接" subtitle={testState() || "用「你好」试一下"}>
                      <SmallButton title="测试" onPress={runTranslateTest} />
                    </Row>
                  </Card>
                </>
              ) : null}
            </>
          ) : current() === 5 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>自定义词</Text>
              <Card>
                <Row title="词">
                  <InputBox
                    value={customWord()}
                    placeholder="例如：ArchType"
                    onChange={(value) => customWord.set(value)}
                  />
                </Row>
                <Row title="编码" subtitle="拼音（空格分开音节），也可以是 mail 这类字母">
                  <InputBox
                    value={customPinyin()}
                    placeholder="例如：a r c h"
                    onChange={(value) => customPinyin.set(value)}
                  />
                </Row>
                <Row title="添加" subtitle={customMessage()}>
                  <SmallButton title="添加" tone="accent" onPress={addCustomWord} />
                </Row>
              </Card>
              <Card>
                {customWords().trim() ? (
                  customWords()
                    .split("\n")
                    .filter((line) => line.trim() !== "" && !line.trimStart().startsWith("#"))
                    .map((line) => {
                      const parts = line.split("\t");
                      const word = parts[1] ?? parts[0];
                      return (
                        <Row title={word} subtitle={`编码 ${parts[0] ?? ""}`}>
                          <SmallButton title="删除" onPress={() => removeCustomWord(word)} />
                        </Row>
                      );
                    })
                ) : (
                  <Row title="还没有自定义词" subtitle="在上面的卡片里添加" />
                )}
              </Card>
            </>
          ) : current() === 7 ? (
            <>
              <Text style={{ color: ui().muted, fontSize: 12 }}>更新</Text>
              <Card>
                <Row title="当前版本" subtitle={Config.version()} />
                <Row title="检查更新" subtitle={updateState() || "从 GitHub 查最新发布"}>
                  <SmallButton title="检查" onPress={runCheckUpdate} />
                </Row>
                {updateUrl() ? (
                  <Row title="打开下载页" subtitle={updateUrl()}>
                    <SmallButton title="打开" onPress={() => Config.openUrl(updateUrl())} />
                  </Row>
                ) : null}
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

mount(App);
