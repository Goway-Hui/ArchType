// 统计页的图表：热力键盘、比例条、条形图、键位小格、指标块。
import { colorScheme, Text, View } from "nsh";
import { ui } from "./theme";

export function formatNumber(value: number): string {
  return Math.round(value)
    .toString()
    .replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

export function Metric(props: { title: string; value: string; unit?: string; note?: string }) {
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

export function heatColor(count: number, max: number): string {
  if (count <= 0 || max <= 0) return colorScheme() === "dark" ? "#343a37" : "#eef1f0";
  const level = Math.max(1, Math.min(4, Math.ceil((count * 4) / max)));
  if (colorScheme() === "dark") {
    return ["#343a37", "rgba(110, 168, 254, 0.22)", "rgba(110, 168, 254, 0.42)",
      "rgba(110, 168, 254, 0.68)", "#6EA8FE"][level];
  }
  return ["#eef1f0", "rgba(47, 111, 237, 0.20)", "rgba(47, 111, 237, 0.40)",
    "rgba(47, 111, 237, 0.65)", "#2F6FED"][level];
}

export function KeyCap(props: { label: string; weight: number; count: number; max: number }) {
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

// 类型 / 模式页的固定分类色,取值与旧版 stats_ratio_page 一致。
export const kindColors: Record<string, string> = {
  han: "#0EAF76",
  latin: "#4C9AFF",
  number: "#F5A623",
  punct: "#8A93A6",
  emoji: "#E86AA6",
  other: "#C4C9C7",
};

export const sourceColors: Record<string, string> = {
  quanpin: "#0EAF76",
  shuangpin: "#4C9AFF",
  english: "#F5A623",
  translate: "#E86AA6",
};

/* 旧版的比例页:一条按份额着色的横条,下面每项色块 + 名称 + 「次数 · 占比」。 */
export function RatioChart(props: {
  parts: { label: string; value: number; color: string }[];
  empty: string;
}) {
  const present = props.parts.filter((part) => part.value > 0);
  if (present.length === 0) {
    return <Text style={{ color: ui().muted, fontSize: 12, paddingTop: 10 }}>{props.empty}</Text>;
  }
  const total = present.reduce((sum, part) => sum + part.value, 0);
  return (
    <View style={{ gap: 8, paddingTop: 10 }}>
      <View style={{ flexDirection: "row", width: "100%", height: 14, borderRadius: 7 }}>
        {present.map((part) => (
          <View style={{ flex: part.value, backgroundColor: part.color }} />
        ))}
      </View>
      {present.map((part) => (
        <View style={{ flexDirection: "row", alignItems: "center", gap: 8 }}>
          <View
            style={{ width: 10, height: 10, borderRadius: 2, backgroundColor: part.color }}
          />
          <Text style={{ color: ui().text, fontSize: 12, flex: 1 }}>{part.label}</Text>
          <Text style={{ color: ui().muted, fontSize: 12 }}>
            {formatNumber(part.value)} · {Math.round((part.value / total) * 100)}%
          </Text>
        </View>
      ))}
    </View>
  );
}

/* 旧版 stats_bar_row:名称 + 横条 + 右侧次数。 */
export function BarRow(props: { label: string; value: number; max: number; suffix: string }) {
  return (
    <View
      style={{ flexDirection: "row", alignItems: "center", gap: 10, paddingTop: 3, paddingBottom: 3 }}
    >
      <Text style={{ color: ui().text, fontSize: 12, width: 96 }}>{props.label}</Text>
      <View style={{ flex: 1, height: 8, borderRadius: 4, backgroundColor: ui().page }}>
        <View
          style={{
            width: `${Math.round((props.value / props.max) * 100)}%`,
            height: 8,
            borderRadius: 4,
            backgroundColor: ui().accent,
          }}
        />
      </View>
      <Text style={{ color: ui().muted, fontSize: 12, width: 64, textAlign: "right" }}>
        {props.suffix}
      </Text>
    </View>
  );
}

export function KeyChip(props: { label: string; count: number; max: number }) {
  return (
    <View
      style={{
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "space-between",
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
      <View style={{ flexDirection: "row", alignItems: "center", gap: 6 }}>
        <View
          style={{
            width: 10,
            height: 10,
            borderRadius: 2,
            backgroundColor: heatColor(props.count, props.max),
          }}
        />
        <Text style={{ color: ui().text, fontSize: 12 }}>{props.label}</Text>
      </View>
      <Text style={{ color: ui().muted, fontSize: 11 }}>{formatNumber(props.count)}</Text>
    </View>
  );
}
