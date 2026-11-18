import { mount, setColorScheme, signal, Scroll, Text, View } from "nsh";
import { NumberInput, Select, Switch } from "nsh/base";
import type { JSX } from "nsh/jsx-runtime";
import { Config } from "archtype/native";

const pages = ["常规", "输入", "表情", "语音", "翻译", "自定义词", "统计", "更新"];

const ui = {
  page: "#f5f7f6",
  sidebar: "#e2ecfc",
  card: "#ffffff",
  border: "#e6eae8",
  text: "#1f2724",
  muted: "#59635e",
  accent: "#2F6FED",
  trackOff: "#cfd5d1",
};

function Card(props: { children: JSX.Element }) {
  return (
    <View
      style={{
        backgroundColor: ui.card,
        borderRadius: 12,
        borderWidth: 1,
        borderColor: ui.border,
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
        <Text style={{ color: ui.text, fontSize: 15 }}>{props.title}</Text>
        {props.subtitle ? (
          <Text style={{ color: ui.muted, fontSize: 12 }}>{props.subtitle}</Text>
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
            borderColor: ui.border,
            backgroundColor: ui.card,
            minWidth: 120,
            justifyContent: "space-between",
          }}
        >
          <Text style={{ color: ui.text, fontSize: 13 }}>
            {props.items.find((item) => item.value === props.value)?.title ?? "选择"}
          </Text>
          <Text style={{ color: ui.muted, fontSize: 10 }}>▾</Text>
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
        backgroundColor: props.checked ? ui.accent : ui.trackOff,
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

  const statsFile = parseStats(Config.stats());
  const days = statsFile.days ?? {};
  const today = days[dayKey(new Date())] ?? {};
  const todayChars = today.chars ?? 0;
  const todayMs = today.activeMs ?? 0;
  const totalChars = Object.values(days).reduce((sum, day) => sum + (day.chars ?? 0), 0);
  const totalKeys = Object.values(days).reduce(
    (sum, day) => sum + Object.values(day.keys ?? {}).reduce((n, count) => n + count, 0),
    0,
  );
  const sources = Object.values(days).reduce<Record<string, number>>((acc, day) => {
    for (const [name, count] of Object.entries(day.sources ?? {})) {
      acc[name] = (acc[name] ?? 0) + count;
    }
    return acc;
  }, {});
  const speed = todayMs > 0 ? Math.round(todayChars / (todayMs / 60000)) : 0;

  const keysMinus = signal(boolOf(Config.get("PageKeysMinusEqual"), true));
  const keysBracket = signal(boolOf(Config.get("PageKeysBracket"), false));
  const keysComma = signal(boolOf(Config.get("PageKeysCommaPeriod"), false));
  const keysTab = signal(boolOf(Config.get("PageKeysTab"), false));

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
    <View style={{ flex: 1, flexDirection: "row", backgroundColor: ui.page }}>
      <View
        style={{
          width: 210,
          backgroundColor: ui.sidebar,
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
              backgroundColor: ui.accent,
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Text style={{ color: "white", fontWeight: "800", fontSize: 14 }}>A</Text>
          </View>
          <Text style={{ color: ui.text, fontWeight: "800", fontSize: 16 }}>ArchType</Text>
          <Text style={{ color: ui.muted, fontSize: 12 }}>设置</Text>
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
              backgroundColor: current() === i ? ui.accent : "transparent",
            }}
          >
            <Text style={{ color: current() === i ? "white" : ui.text, fontSize: 14 }}>{p}</Text>
          </View>
        ))}
      </View>

      <Scroll style={{ flex: 1 }}>
        <View style={{ padding: 28, gap: 16 }}>
          <Text style={{ color: ui.text, fontWeight: "800", fontSize: 24 }}>{pages[current()]}</Text>

          {current() === 0 ? (
            <>
              <Text style={{ color: ui.muted, fontSize: 12 }}>候选词</Text>
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

              <Text style={{ color: ui.muted, fontSize: 12 }}>翻页键</Text>
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

              <Text style={{ color: ui.muted, fontSize: 12 }}>外观</Text>
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

              <Text style={{ color: ui.muted, fontSize: 12 }}>
                配置文件：{Config.configPath()}
              </Text>
            </>
          ) : current() === 1 ? (
            <>
              <Text style={{ color: ui.muted, fontSize: 12 }}>输入方案</Text>
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

              <Text style={{ color: ui.muted, fontSize: 12 }}>模糊音</Text>
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

              <Text style={{ color: ui.muted, fontSize: 12 }}>其他</Text>
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

              <Text style={{ color: ui.muted, fontSize: 12 }}>
                词库：{Config.get("Lexicon") || "雾凇拼音（默认）"}
              </Text>
            </>
          ) : current() === 2 ? (
            <>
              <Text style={{ color: ui.muted, fontSize: 12 }}>表情</Text>
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
              <Text style={{ color: ui.muted, fontSize: 12 }}>打字统计</Text>
              <Card>
                <Row title="今日输入" subtitle={`${todayChars} 字符`} />
                <Row title="累计输入" subtitle={`${totalChars} 字符`} />
                <Row title="今日活跃" subtitle={`${Math.round(todayMs / 60000)} 分钟`} />
                <Row title="今日速度" subtitle={`${speed} 字/分钟`} />
                <Row title="累计按键" subtitle={`${totalKeys} 次`} />
              </Card>
              <Text style={{ color: ui.muted, fontSize: 12 }}>输入来源</Text>
              <Card>
                <Row title="全拼" subtitle={`${sources.quanpin ?? 0} 次`} />
                <Row title="双拼" subtitle={`${sources.shuangpin ?? 0} 次`} />
                <Row title="英文" subtitle={`${sources.english ?? 0} 次`} />
                <Row title="互译" subtitle={`${sources.translate ?? 0} 次`} />
              </Card>
            </>
          ) : current() === 3 ? (
            <>
              <Text style={{ color: ui.muted, fontSize: 12 }}>语音</Text>
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
              <Text style={{ color: ui.muted, fontSize: 12 }}>
                识别模型：{voiceModel() || "未下载（在语音设置里下载模型后可用）"}
              </Text>
            </>
          ) : current() === 4 ? (
            <>
              <Text style={{ color: ui.muted, fontSize: 12 }}>翻译</Text>
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
                <Text style={{ color: ui.muted, fontSize: 12 }}>
                  自定义接口：{translateBase() || "未填写（编辑功能迁移中）"}
                </Text>
              ) : null}
            </>
          ) : current() === 5 ? (
            <>
              <Text style={{ color: ui.muted, fontSize: 12 }}>自定义词</Text>
              <Card>
                {customWords() ? (
                  customWords()
                    .split("\n")
                    .filter((line) => line.trim() !== "")
                    .slice(0, 40)
                    .map((line) => (
                      <Row title={line.split("\t")[0]} subtitle={line.split("\t")[1] ?? ""} />
                    ))
                ) : (
                  <Row title="还没有自定义词" subtitle="在设置里添加后显示在这里" />
                )}
              </Card>
            </>
          ) : current() === 7 ? (
            <>
              <Text style={{ color: ui.muted, fontSize: 12 }}>更新</Text>
              <Card>
                <Row title="当前版本" subtitle={Config.version()} />
                <Row title="检查更新" subtitle="检查更新和下载安装正在迁移中" />
              </Card>
            </>
          ) : (
            <Card>
              <Row title={pages[current()]} subtitle="这一页的设置在逐步迁移中" />
            </Card>
          )}

          <Text style={{ color: ui.muted, fontSize: 12 }}>
            ArchType 设置（NSH 预览版，版本 {Config.version()}）
          </Text>
        </View>
      </Scroll>
    </View>
  );
}

mount(App);
