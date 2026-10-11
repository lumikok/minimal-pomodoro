export function formatCountdown(seconds: number): string {
  const total = Math.max(0, Math.ceil(seconds));
  const minutes = Math.floor(total / 60), remainder = String(total % 60).padStart(2, "0");
  return total >= 3600 ? `${Math.floor(total / 3600)}:${String(minutes % 60).padStart(2, "0")}:${remainder}` : `${String(minutes).padStart(2, "0")}:${remainder}`;
}
export function formatFocusTime(minutes: number): string {
  if (minutes < 60) return `${minutes} 分钟`;
  const hours = Math.floor(minutes / 60), remainder = minutes % 60;
  return remainder ? `${hours} 小时 ${remainder} 分钟` : `${hours} 小时`;
}
