import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./style.css";

type Phase = "focus" | "short_break" | "long_break";
type Status = "idle" | "running" | "paused" | "completed";
interface Settings {
  focusMinutes: number; shortBreakMinutes: number; longBreakMinutes: number;
  soundEnabled: boolean; alwaysOnTop: boolean;
}
interface Snapshot {
  phase: Phase; status: Status; remainingSeconds: number; totalSeconds: number;
  nextPhase: Phase; settings: Settings;
  today: { date: string; completedCount: number; focusMinutes: number };
  cycleCount: number; trayHintSeen: boolean; warning: string | null; revision: number;
}
const labels: Record<Phase, string> = { focus: "专注", short_break: "短休息", long_break: "长休息" };
const iconPin = `<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m9 3 6 0-1 6 4 4H6l4-4-1-6ZM12 13v8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const iconSettings = `<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M4 7h16M4 17h16M8 4v6M16 14v6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>`;
document.querySelector<HTMLElement>("#app")!.innerHTML = `
  <header class="header">
    <div class="brand"><span class="brand-dot" aria-hidden="true"></span><h1>极简番茄钟</h1></div>
    <div class="header-actions">
      <button id="pin" class="icon-button" title="置顶窗口" aria-label="置顶窗口" aria-pressed="false">${iconPin}</button>
      <button id="settings-toggle" class="icon-button" title="计时设置" aria-label="计时设置" aria-expanded="false" aria-controls="settings-panel">${iconSettings}</button>
    </div>
  </header>
  <nav class="phases" aria-label="计时阶段">
    <button data-phase="focus" aria-pressed="true">专注</button>
    <button data-phase="short_break" aria-pressed="false">短休息</button>
    <button data-phase="long_break" aria-pressed="false">长休息</button>
  </nav>
  <section class="timer" aria-label="倒计时">
    <p id="status-label" class="eyebrow">准备开始</p>
    <div id="time" class="time" role="timer" aria-live="off" aria-label="剩余25分钟">25:00</div>
    <div class="progress-track" aria-hidden="true"><div id="progress" class="progress-fill"></div></div>
    <p id="message" class="message" role="status">给这一段学习，留一点安静。</p>
  </section>
  <div id="presets" class="presets" aria-label="专注时长预设">
    <button data-minutes="25" aria-pressed="true">25 分钟</button><button data-minutes="50" aria-pressed="false">50 分钟</button><button id="custom">自定义</button>
  </div>
  <div class="timer-actions"><button id="primary" class="primary">开始专注 →</button><button id="reset" class="secondary">重置</button></div>
  <button id="skip-break" class="text-button" hidden>跳过休息，开始专注</button>
  <footer class="summary" aria-label="今日学习统计">
    <div><span class="summary-label">今日完成</span><strong id="count">0 <small>次</small></strong></div>
    <span class="summary-divider" aria-hidden="true"></span>
    <div><span class="summary-label">已完成专注</span><strong id="minutes">0 <small>分钟</small></strong></div>
  </footer>
  <section id="settings-panel" class="settings-panel" hidden aria-label="计时设置">
    <div class="section-heading"><h2>按你的节奏来</h2><span>1–180 分钟</span></div>
    <form id="settings-form">
      <div class="duration-fields">
        <label>专注<input id="focus-input" type="number" min="1" max="180" step="1" required value="25" inputmode="numeric" /></label>
        <label>短休息<input id="short-input" type="number" min="1" max="180" step="1" required value="5" inputmode="numeric" /></label>
        <label>长休息<input id="long-input" type="number" min="1" max="180" step="1" required value="15" inputmode="numeric" /></label>
      </div>
      <label class="sound-option"><input id="sound-input" type="checkbox" checked /> 到点播放短提示音</label>
      <p class="settings-note">运行中的计时保持原时长，修改从下一段生效。</p>
      <button class="save-button" type="submit">保存设置</button>
    </form>
    <p class="local-note">离线使用 · 学习复盘交给拾光</p>
  </section>
  <p id="feedback" class="feedback" role="status" hidden></p><p id="warning" class="warning" role="alert" hidden></p>
  <dialog id="confirm-dialog" aria-labelledby="dialog-title" aria-describedby="dialog-message">
    <form method="dialog"><h2 id="dialog-title">重新开始这一段？</h2><p id="dialog-message">当前未完成的计时会被放弃，不计入今日统计。</p>
      <div class="dialog-actions"><button value="cancel" class="secondary">取消</button><button value="confirm" class="primary">确认</button></div>
    </form>
  </dialog>`;

const el = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
function updateText(id: string, text: string) { if (el(id).textContent !== text) el(id).textContent = text; }
let state: Snapshot | null = null;
let busy = false, desktopReady = false, settingsDirty = false;
let feedbackTimeout: ReturnType<typeof setTimeout>;
function feedback(message: string) {
  clearTimeout(feedbackTimeout);
  el("feedback").textContent = message; el("feedback").hidden = false;
  feedbackTimeout = setTimeout(() => { el("feedback").hidden = true; }, 4500);
}
function syncSettings(settings: Settings) {
  el<HTMLInputElement>("focus-input").value = String(settings.focusMinutes);
  el<HTMLInputElement>("short-input").value = String(settings.shortBreakMinutes);
  el<HTMLInputElement>("long-input").value = String(settings.longBreakMinutes);
  el<HTMLInputElement>("sound-input").checked = settings.soundEnabled;
}
function render(next: Snapshot) {
  if (state && next.revision < state.revision) return;
  const previous = state; state = next;
  if (!settingsDirty && (!previous || JSON.stringify(previous.settings) !== JSON.stringify(next.settings))) syncSettings(next.settings);
  const minutes = Math.floor(next.remainingSeconds / 60), seconds = next.remainingSeconds % 60;
  updateText("time", `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`);
  el("time").setAttribute("aria-label", `剩余${minutes}分钟${seconds}秒`);
  const titles: Record<Status, string> = { idle: "准备开始", running: `${labels[next.phase]}中`, paused: "已暂停", completed: `${labels[next.phase]}完成` };
  updateText("status-label", titles[next.status]);
  el("progress").style.width = `${Math.min(100, Math.max(0, (1 - next.remainingSeconds / next.totalSeconds) * 100))}%`;
  el("pin").setAttribute("aria-pressed", String(next.settings.alwaysOnTop));
  el("pin").title = next.settings.alwaysOnTop ? "取消置顶" : "置顶窗口";
  const messages: Record<Status, string> = {
    idle: next.phase === "focus" ? "给这一段学习，留一点安静。" : "放松一下，下一段会更清晰。",
    running: next.phase === "focus" ? "只做眼前这一件事。" : "离开屏幕，活动一下。",
    paused: "缓一缓，准备好再继续。",
    completed: next.phase === "focus" ? (next.nextPhase === "long_break" ? "已完成四轮，给自己一段长休息。" : "这一段完成了，休息由你开始。") : "休息结束，准备好再开始学习。",
  };
  updateText("message", messages[next.status]);
  updateText("primary", next.status === "running" ? "暂停" : next.status === "paused" ? "继续" : `开始${labels[next.status === "completed" ? next.nextPhase : next.phase]} →`);
  for (const button of document.querySelectorAll<HTMLButtonElement>("#primary, #pin, #custom, #skip-break, [data-phase], [data-minutes], .save-button")) button.disabled = busy || !desktopReady;
  el<HTMLButtonElement>("reset").disabled = busy || !desktopReady || next.status === "idle";
  for (const button of document.querySelectorAll<HTMLButtonElement>("[data-phase]")) button.setAttribute("aria-pressed", String(button.dataset.phase === next.phase));
  for (const button of document.querySelectorAll<HTMLButtonElement>("[data-minutes]")) button.setAttribute("aria-pressed", String(Number(button.dataset.minutes) === next.settings.focusMinutes));
  el("presets").hidden = next.phase !== "focus";
  el("skip-break").hidden = !(next.status === "completed" && next.phase === "focus");
  if (!previous || previous.today.completedCount !== next.today.completedCount) el("count").replaceChildren(String(next.today.completedCount), Object.assign(document.createElement("small"), { textContent: " 次" }));
  if (!previous || previous.today.focusMinutes !== next.today.focusMinutes) el("minutes").replaceChildren(String(next.today.focusMinutes), Object.assign(document.createElement("small"), { textContent: " 分钟" }));
  updateText("warning", next.warning ?? ""); el("warning").hidden = !next.warning;
  const title = `${el("time").textContent} · ${labels[next.phase]} · 极简番茄钟`;
  if (document.title !== title) document.title = title;
}
async function command(name: string, args?: Record<string, unknown>) {
  const result = await invoke<Snapshot>(name, args); render(result); return result;
}
async function action(operation: () => Promise<unknown>) {
  if (busy || !desktopReady) return;
  busy = true; if (state) render(state);
  try { await operation(); } catch (error) { feedback(String(error)); }
  finally { busy = false; if (state) render(state); }
}
function confirmDiscard(title: string, message = "当前未完成的计时会被放弃，不计入今日统计。") {
  const dialog = el<HTMLDialogElement>("confirm-dialog");
  el("dialog-title").textContent = title; el("dialog-message").textContent = message;
  return new Promise<boolean>((resolve) => {
    dialog.returnValue = "cancel";
    dialog.addEventListener("close", () => resolve(dialog.returnValue === "confirm"), { once: true });
    dialog.showModal();
  });
}
function isStarted() { return state?.status === "running" || state?.status === "paused"; }
el("primary").addEventListener("click", () => action(async () => {
  await command(state?.status === "running" ? "pause_timer" : state?.status === "paused" ? "resume_timer" : "start_timer");
}));
el("reset").addEventListener("click", () => action(async () => {
  if (isStarted() && !await confirmDiscard("重新开始这一段？")) return;
  await command("reset_timer", { confirmed: true });
}));
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-phase]")) button.addEventListener("click", () => action(async () => {
  const phase = button.dataset.phase as Phase;
  if (phase === state?.phase && state.status !== "completed") return;
  if (isStarted() && !await confirmDiscard(`切换到${labels[phase]}？`)) return;
  await command("switch_phase", { phase, confirmed: true });
}));
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-minutes]")) button.addEventListener("click", () => action(async () => {
  if (!state) return;
  await command("save_settings", { settings: { ...state.settings, focusMinutes: Number(button.dataset.minutes) } });
  feedback(isStarted() || state.status === "completed" ? "专注时长已保存，从下一段生效。" : "专注时长已调整。");
}));
el("skip-break").addEventListener("click", () => action(async () => {
  await command("switch_phase", { phase: "focus", confirmed: true }); await command("start_timer");
}));
el("pin").addEventListener("click", () => action(async () => {
  if (state) await command("set_always_on_top", { enabled: !state.settings.alwaysOnTop });
}));
function showSettings(open: boolean) {
  el("settings-panel").hidden = !open; el("settings-toggle").setAttribute("aria-expanded", String(open));
  if (open) el("settings-panel").scrollIntoView({ behavior: "instant", block: "nearest" });
}
el("settings-toggle").addEventListener("click", () => showSettings(el("settings-panel").hidden));
el("custom").addEventListener("click", () => { showSettings(true); el<HTMLInputElement>("focus-input").focus(); });
el("settings-form").addEventListener("input", () => { settingsDirty = true; });
el("settings-form").addEventListener("submit", (event) => {
  event.preventDefault(); void action(async () => {
    if (!state) return;
    const values = ["focus-input", "short-input", "long-input"].map((id) => Number(el<HTMLInputElement>(id).value));
    if (values.some((value) => !Number.isInteger(value) || value < 1 || value > 180)) throw new Error("请输入 1–180 的整数分钟。");
    const settings = { ...state.settings, focusMinutes: values[0], shortBreakMinutes: values[1], longBreakMinutes: values[2], soundEnabled: el<HTMLInputElement>("sound-input").checked };
    await command("save_settings", { settings }); settingsDirty = false; syncSettings(state.settings); feedback("设置已保存。");
  });
});
async function initialize() {
  try {
    await listen<Snapshot>("timer-state", (event) => render(event.payload));
    await listen("tray-hint", () => { void action(async () => {
      if (await confirmDiscard("关闭窗口会收进托盘", "计时会继续，到点照常提醒。点击托盘图标可重新打开；在托盘菜单选择「退出」才会结束应用。")) await command("acknowledge_tray_hint");
    }); });
    desktopReady = true; await command("get_state");
  } catch (error) {
    desktopReady = false;
    // Browser preview intentionally contains no second timer engine.
    render({ phase: "focus", status: "idle", remainingSeconds: 1500, totalSeconds: 1500, nextPhase: "short_break", settings: { focusMinutes: 25, shortBreakMinutes: 5, longBreakMinutes: 15, soundEnabled: true, alwaysOnTop: false }, today: { date: "", completedCount: 0, focusMinutes: 0 }, cycleCount: 0, trayHintSeen: false, warning: null, revision: 0 });
    feedback("请在桌面应用中使用计时。浏览器仅用于预览界面。"); console.error("Desktop initialization failed", error);
  }
}
void initialize();
