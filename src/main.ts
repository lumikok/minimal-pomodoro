import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatCountdown, formatFocusTime } from "./format";
import "./style.css";

type Phase = "focus" | "short_break" | "long_break";
type Status = "idle" | "running" | "paused" | "completed";
interface Counters { completedCount: number; focusMinutes: number }
interface Settings { focusMinutes: number; shortBreakMinutes: number; longBreakMinutes: number; soundEnabled: boolean; alwaysOnTop: boolean }
interface Snapshot {
  phase: Phase; status: Status; remainingSeconds: number; totalSeconds: number; nextPhase: Phase; settings: Settings;
  today: Counters & { date: string }; lifetime: Counters & { since: string; history: Counters };
  window: { opacity: number; position: { x: number; y: number } | null };
  cycleCount: number; trayHintSeen: boolean; warning: string | null; revision: number; reminderPending: boolean; expanded: boolean; ringing: boolean;
}
const labels: Record<Phase, string> = { focus: "专注", short_break: "短休息", long_break: "长休息" };
const el = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
document.querySelector<HTMLDivElement>("#app")!.innerHTML = `
  <section class="mini" aria-label="倒计时浮窗">
    <div class="mini-clock" data-drag><span id="mini-label">专注 · 待开始</span><strong id="mini-time">25:00</strong></div>
    <div class="mini-controls"><button id="mini-primary" title="开始专注" aria-label="开始专注">▶</button><button id="expand" title="展开设置和统计" aria-label="展开设置和统计">⌄</button></div>
    <span id="mini-warning" hidden title="请展开查看保存提示">!</span>
  </section>
  <section class="notice" role="alert" aria-label="计时完成提醒" hidden>
    <div class="notice-top" data-drag><span>极简番茄钟</span><button id="quiet" aria-label="停止铃声" title="停止铃声">静音</button></div>
    <h1 id="notice-title">专注完成</h1><p id="notice-body">这一段完成了，起来活动一下。</p>
    <div class="notice-actions"><button id="notice-start" class="primary">开始短休息</button><button id="later" class="secondary">稍后</button></div>
    <p class="notice-foot">下一段由你开始 · 提示会保留到确认</p>
  </section>
  <section class="panel" hidden>
    <header data-drag><strong>极简番茄钟</strong><div class="header-actions"><button id="pin" aria-label="置顶窗口" title="置顶窗口">置顶</button><button id="collapse" title="收起面板" aria-label="收起面板">⌃</button><button id="hide" title="收进托盘，计时继续" aria-label="收进托盘">×</button></div></header>
    <nav class="phases" aria-label="计时阶段"><button data-phase="focus">专注</button><button data-phase="short_break">短休息</button><button data-phase="long_break">长休息</button></nav>
    <div class="timer"><span id="status-label">准备开始</span><div id="time" role="timer" aria-live="off">25:00</div><div class="progress"><div id="progress"></div></div></div>
    <div class="presets" id="presets"><button data-minutes="25">25 分钟</button><button data-minutes="50">50 分钟</button><button id="custom">自定义</button></div>
    <div class="actions"><button id="primary" class="primary">开始专注</button><button id="reset" class="secondary">重置</button></div>
    <button id="skip-break" class="text-button" hidden>跳过休息，开始专注</button>
    <section class="stats" aria-label="学习统计"><div class="stats-tabs"><button id="total-tab" aria-pressed="true">累计</button><button id="today-tab" aria-pressed="false">今日</button></div><div class="summary"><div><span>完成专注</span><strong id="count">0 次</strong></div><div><span>专注时长</span><strong id="minutes">0 分钟</strong></div></div><p id="since" class="note"></p></section>
    <details id="settings"><summary>计时与浮窗设置</summary><form id="settings-form"><div class="fields"><label>专注（分钟）<input id="focus-input" type="number" min="1" max="180" required></label><label>短休息<input id="short-input" type="number" min="1" max="180" required></label><label>长休息<input id="long-input" type="number" min="1" max="180" required></label></div><label class="check"><input id="sound-input" type="checkbox"> 到点播放约 10 秒铃声</label><p class="note">时长修改从下一段生效。</p><button class="save-button" type="submit">保存计时设置</button></form><form id="opacity-form"><label class="opacity-label">底色不透明度 <output id="opacity-value">80%</output><input id="opacity-input" type="range" min="40" max="100" value="80"></label><button type="submit" class="save-button">保存浮窗底色</button></form></details>
    <details id="history"><summary>补录旧版未保留的历史</summary><p class="note">仅填已丢失的历史部分，不包含上面的现存统计。旧版只保留当天，往日记录无法自动恢复。修改会替换原补录值。</p><form id="history-form"><div class="fields history-fields"><label>完成次数<input id="history-count" type="number" min="0" max="1000000000" value="0" required></label><label>专注分钟<input id="history-minutes" type="number" min="0" max="180000000000" value="0" required></label></div><button class="save-button" type="submit">保存历史补录</button></form></details>
    <p id="warning" class="warning" hidden></p><p id="feedback" class="feedback" hidden></p>
    <p class="foot">拖动顶部可移动 · 收起后继续显示倒计时</p>
  </section>
  <dialog id="confirm-dialog"><form method="dialog"><h2 id="dialog-title">放弃这一段？</h2><p>未完成的计时不会计入统计。</p><div class="actions"><button value="cancel" class="secondary">取消</button><button value="confirm" class="primary">确认</button></div></form></dialog>`;

let state: Snapshot | null = null, busy = false, ready = false, dirty = false, historyDirty = false, opacityDirty = false, totalView = true;
function text(id: string, value: string) { if (el(id).textContent !== value) el(id).textContent = value; }
function feedback(value: string) { text("feedback", value); el("feedback").hidden = false; }
function render(next: Snapshot) {
  if (state && next.revision < state.revision) return;
  state = next;
  const mode = next.reminderPending ? "notice" : next.expanded ? "panel" : "mini";
  document.body.dataset.mode = mode;
  for (const name of ["mini", "notice", "panel"]) (document.querySelector(`.${name}`) as HTMLElement).hidden = name !== mode;
  document.documentElement.style.setProperty("--surface-alpha", String(next.window.opacity / 100));
  const remaining = formatCountdown(next.remainingSeconds);
  text("time", remaining); text("mini-time", remaining);
  el("mini-time").classList.toggle("hours", next.remainingSeconds >= 3600);
  const status = { idle: "待开始", running: "计时中", paused: "已暂停", completed: "已完成" }[next.status];
  text("mini-label", `${labels[next.phase]} · ${status}`); text("status-label", `${labels[next.phase]} · ${status}`);
  el("time").setAttribute("aria-label", `剩余 ${remaining}`);
  el("progress").style.width = `${Math.min(100, Math.max(0, 100 * (1 - next.remainingSeconds / next.totalSeconds)))}%`;
  const primary = next.status === "running" ? "暂停" : next.status === "paused" ? "继续" : `开始${labels[next.status === "completed" ? next.nextPhase : next.phase]}`;
  text("primary", primary); text("mini-primary", next.status === "running" ? "Ⅱ" : "▶");
  el("mini-primary").title = primary; el("mini-primary").setAttribute("aria-label", primary);
  text("notice-title", `${labels[next.phase]}完成`);
  text("notice-body", next.phase === "focus" ? (next.nextPhase === "long_break" ? "四轮专注完成，给自己一段长休息。" : "这一段完成了，起来活动一下。") : "休息结束，准备好再开始学习。");
  text("notice-start", `开始${labels[next.nextPhase]}`);
  el("pin").setAttribute("aria-pressed", String(next.settings.alwaysOnTop));
  el("pin").title = next.settings.alwaysOnTop ? "取消置顶" : "置顶窗口";
  for (const button of document.querySelectorAll<HTMLButtonElement>("[data-phase]")) button.setAttribute("aria-pressed", String(button.dataset.phase === next.phase));
  for (const button of document.querySelectorAll<HTMLButtonElement>("[data-minutes]")) button.setAttribute("aria-pressed", String(Number(button.dataset.minutes) === next.settings.focusMinutes));
  for (const button of document.querySelectorAll<HTMLButtonElement>(".mini button,.panel button,.notice button")) button.disabled = busy || !ready;
  el<HTMLButtonElement>("reset").disabled = busy || !ready || next.status === "idle";
  el<HTMLButtonElement>("quiet").disabled = busy || !ready || !next.ringing;
  el("presets").hidden = next.phase !== "focus";
  el("skip-break").hidden = !(next.status === "completed" && next.phase === "focus");
  const count = totalView ? next.lifetime.completedCount + next.lifetime.history.completedCount : next.today.completedCount;
  const minutes = totalView ? next.lifetime.focusMinutes + next.lifetime.history.focusMinutes : next.today.focusMinutes;
  text("count", `${count} 次`); text("minutes", formatFocusTime(minutes));
  text("since", totalView ? `现存记录自 ${next.lifetime.since}${next.lifetime.history.completedCount ? " · 含历史补录" : ""}` : next.today.date);
  el("total-tab").setAttribute("aria-pressed", String(totalView)); el("today-tab").setAttribute("aria-pressed", String(!totalView));
  if (!dirty) {
    el<HTMLInputElement>("focus-input").value = String(next.settings.focusMinutes); el<HTMLInputElement>("short-input").value = String(next.settings.shortBreakMinutes);
    el<HTMLInputElement>("long-input").value = String(next.settings.longBreakMinutes); el<HTMLInputElement>("sound-input").checked = next.settings.soundEnabled;
  }
  if (!historyDirty) { el<HTMLInputElement>("history-count").value = String(next.lifetime.history.completedCount); el<HTMLInputElement>("history-minutes").value = String(next.lifetime.history.focusMinutes); }
  if (!opacityDirty) { el<HTMLInputElement>("opacity-input").value = String(next.window.opacity); text("opacity-value", `${next.window.opacity}%`); }
  text("warning", next.warning ?? ""); el("warning").hidden = !next.warning; el("mini-warning").hidden = !next.warning;
  document.title = `${remaining} · ${labels[next.phase]} · 极简番茄钟`;
}
async function command(name: string, args?: Record<string, unknown>) { const next = await invoke<Snapshot>(name, args); render(next); return next; }
async function action(operation: () => Promise<unknown>) {
  if (busy || !ready) return;
  busy = true; if (state) render(state);
  try { await operation(); } catch (error) { feedback(String(error)); if (state && !state.reminderPending) await command("set_expanded", { expanded: true }); }
  finally { busy = false; if (state) render(state); }
}
function confirmDiscard(title: string) {
  const dialog = el<HTMLDialogElement>("confirm-dialog"); text("dialog-title", title);
  return new Promise<boolean>((resolve) => { dialog.returnValue = "cancel"; dialog.addEventListener("close", () => resolve(dialog.returnValue === "confirm"), { once: true }); dialog.showModal(); });
}
const started = () => state?.status === "running" || state?.status === "paused";
for (const id of ["primary", "mini-primary", "notice-start"]) el(id).addEventListener("click", () => action(() => command(state?.status === "running" ? "pause_timer" : state?.status === "paused" ? "resume_timer" : "start_timer")));
el("expand").addEventListener("click", () => action(() => command("set_expanded", { expanded: true })));
el("collapse").addEventListener("click", () => action(() => command("set_expanded", { expanded: false })));
el("hide").addEventListener("click", () => action(() => command("hide_window")));
el("later").addEventListener("click", () => action(() => command("acknowledge_completion")));
el("quiet").addEventListener("click", () => action(() => command("silence_chime")));
el("pin").addEventListener("click", () => action(() => command("set_always_on_top", { enabled: !state?.settings.alwaysOnTop })));
el("reset").addEventListener("click", () => action(async () => { if (started() && !await confirmDiscard("重新开始这一段？")) return; await command("reset_timer", { confirmed: true }); }));
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-phase]")) button.addEventListener("click", () => action(async () => {
  if (button.dataset.phase === state?.phase && state.status !== "completed") return;
  if (started() && !await confirmDiscard("切换阶段并放弃这一段？")) return;
  await command("switch_phase", { phase: button.dataset.phase, confirmed: true });
}));
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-minutes]")) button.addEventListener("click", () => action(async () => {
  if (state) await command("save_settings", { settings: { ...state.settings, focusMinutes: Number(button.dataset.minutes) } });
}));
el("skip-break").addEventListener("click", () => action(async () => { await command("switch_phase", { phase: "focus", confirmed: true }); await command("start_timer"); }));
for (const [id, showTotal] of [["total-tab", true], ["today-tab", false]] as const) el(id).addEventListener("click", () => { totalView = showTotal; if (state) render(state); });
el("custom").addEventListener("click", () => { el<HTMLDetailsElement>("settings").open = true; el("settings").scrollIntoView({ block: "nearest" }); el<HTMLInputElement>("focus-input").focus(); });
el("settings-form").addEventListener("input", () => { dirty = true; });
el("settings-form").addEventListener("submit", event => { event.preventDefault(); void action(async () => {
  if (!state) return;
  const values = ["focus-input", "short-input", "long-input"].map(id => Number(el<HTMLInputElement>(id).value));
  if (values.some(v => !Number.isInteger(v) || v < 1 || v > 180)) throw Error("请输入 1–180 的整数分钟。");
  await command("save_settings", { settings: { ...state.settings, focusMinutes: values[0], shortBreakMinutes: values[1], longBreakMinutes: values[2], soundEnabled: el<HTMLInputElement>("sound-input").checked } }); dirty = false; feedback("已保存，时长从下一段生效。");
}); });
el("history-form").addEventListener("input", () => { historyDirty = true; });
el("history-form").addEventListener("submit", event => { event.preventDefault(); void action(async () => {
  const completedCount = Number(el<HTMLInputElement>("history-count").value), focusMinutes = Number(el<HTMLInputElement>("history-minutes").value);
  if (![completedCount, focusMinutes].every(Number.isSafeInteger) || completedCount < 0 || completedCount > 1e9 || focusMinutes < completedCount || focusMinutes > completedCount * 180) throw Error("请输入匹配的次数与分钟（每次 1–180 分钟）。");
  await command("save_history", { history: { completedCount, focusMinutes } }); historyDirty = false; feedback("历史补录已替换保存，不会重复累加。");
}); });
el("opacity-input").addEventListener("input", () => { opacityDirty = true; text("opacity-value", `${el<HTMLInputElement>("opacity-input").value}%`); });
el("opacity-form").addEventListener("submit", event => { event.preventDefault(); void action(async () => { await command("save_opacity", { opacity: Number(el<HTMLInputElement>("opacity-input").value) }); opacityDirty = false; }); });
for (const drag of document.querySelectorAll<HTMLElement>("[data-drag]")) drag.addEventListener("mousedown", event => {
  if (event.button === 0 && !(event.target as HTMLElement).closest("button,input")) { void getCurrentWindow().startDragging().catch(error => feedback(String(error))); }
});
async function initialize() {
  try { await listen<Snapshot>("timer-state", event => render(event.payload)); ready = true; await command("get_state"); }
  catch (error) {
    ready = false;
    render({ phase: "focus", status: "idle", remainingSeconds: 1500, totalSeconds: 1500, nextPhase: "short_break", settings: { focusMinutes: 25, shortBreakMinutes: 5, longBreakMinutes: 15, soundEnabled: true, alwaysOnTop: true }, today: { date: "", completedCount: 0, focusMinutes: 0 }, lifetime: { since: "", completedCount: 0, focusMinutes: 0, history: { completedCount: 0, focusMinutes: 0 } }, window: { opacity: 80, position: null }, cycleCount: 0, trayHintSeen: false, warning: null, revision: 0, reminderPending: false, expanded: false, ringing: false });
    console.error(error);
  }
}
void initialize();
