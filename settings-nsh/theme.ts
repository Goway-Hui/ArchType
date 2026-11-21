// 设置界面的配色：浅色/深色两套，ui() 跟着系统主题切换。
// 样式里写成 ui().xxx 的函数/属性只在渲染时读，主题一换就重算。
import { colorScheme, computed } from "nsh";

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

export const ui = computed(() => (colorScheme() === "dark" ? darkUi : lightUi));
