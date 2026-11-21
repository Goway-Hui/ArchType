// 设置界面复用的基础控件：卡片、一行设置、下拉、开关、小按钮……
// 样式都从 theme.ts 的 ui() 取，不在这里写死颜色。
import { Text, View } from "nsh";
import { Button, Input, Select, Switch } from "nsh/base";
import type { JSX } from "nsh/jsx-runtime";
import { ui } from "./theme";

export function SmallButton(props: {
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

export function InputBox(props: {
  value: string;
  placeholder?: string;
  masked?: boolean;
  width?: number;
  suffix?: JSX.Element;
  onChange: (value: string) => void;
}) {
  return (
    <View
      style={{
        flexDirection: "row",
        alignItems: "center",
        gap: 4,
        backgroundColor: ui().page,
        borderWidth: 1,
        borderColor: ui().border,
        borderRadius: 8,
        width: props.width ?? 240,
        flexShrink: 0,
        paddingRight: props.suffix ? 6 : 0,
      }}
    >
      <Input
        value={props.value}
        placeholder={props.placeholder}
        masked={props.masked}
        style={{
          flex: 1,
          color: ui().text,
          paddingLeft: 10,
          paddingRight: 10,
          paddingTop: 6,
          paddingBottom: 6,
          fontSize: 13,
        }}
        onChange={(event) => props.onChange(event.value)}
      />
      {props.suffix ? props.suffix : null}
    </View>
  );
}

export function Card(props: { children: JSX.Element }) {
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

export function Row(props: { title: string; subtitle?: string; children?: JSX.Element }) {
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

export function SelectBox(props: {
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

// 标题右侧的窗口按钮:圆形浅底,和旧版 HeaderBar 的三个钮一样。
export function WindowButton(props: { title: string; onPress: () => void; children: JSX.Element }) {
  return (
    <View
      accessibilityLabel={props.title}
      onPress={props.onPress}
      style={{
        width: 26,
        height: 26,
        borderRadius: 13,
        backgroundColor: ui().border,
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {props.children}
    </View>
  );
}

// 带加减号的数字输入：nsh 的 NumberInput 把数字夹在两个按钮中间，自己排一行。
export function StepInput(props: {
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

export function RadioDot(props: { selected: boolean }) {
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

export function Toggle(props: { checked: boolean; disabled?: boolean; onChange: (on: boolean) => void }) {
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
