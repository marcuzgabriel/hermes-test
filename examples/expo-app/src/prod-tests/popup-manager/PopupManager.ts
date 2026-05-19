// Real production logic — PopupManager state machine
// Simplified but preserves the key patterns: priority queue, quarantine, cache

interface PopupConfig { id: string; priority: number; condition: () => boolean; }
interface PopupResult { id: string; priority: number; }

const cache = new Map<string, number>(); // id → quarantine expiry timestamp

export class PopupManager {
  private static configs: PopupConfig[] = [];

  static register(config: PopupConfig) { this.configs.push(config); }

  static cleanCache() { cache.clear(); }

  static setQuarantine(id: string, durationMs: number) {
    cache.set(id, Date.now() + durationMs);
  }

  static isQuarantined(id: string): boolean {
    const expiry = cache.get(id);
    if (!expiry) return false;
    if (Date.now() >= expiry) { cache.delete(id); return false; }
    return true;
  }

  static getPopups(): PopupResult[] {
    return this.configs
      .filter(c => c.condition() && !this.isQuarantined(c.id))
      .sort((a, b) => b.priority - a.priority)
      .map(c => ({ id: c.id, priority: c.priority }));
  }

  static reset() {
    this.configs = [];
    this.cleanCache();
  }
}
